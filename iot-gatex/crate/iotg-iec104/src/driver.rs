use std::time::{Duration, Instant};

use async_trait::async_trait;
use bytes::BytesMut;
use iotg_core::{Batch, Driver};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::mpsc,
    time::{sleep, timeout},
};
use tracing::{debug, error, info, warn};

use crate::{
    config::Iec104Config,
    protocol::{
        apci::{Frame, UType},
        asdu,
    },
};

pub struct Iec104Driver {
    pub cfg: Iec104Config,
}

impl Iec104Driver {
    pub fn new(cfg: Iec104Config) -> Self { Self { cfg } }
}

#[async_trait]
impl Driver for Iec104Driver {
    fn protocol(&self) -> &'static str { "iec104" }
    fn name(&self) -> &str { &self.cfg.name }

    async fn run(self: Box<Self>, tx: mpsc::Sender<Batch>) {
        let addr = format!("{}:{}", self.cfg.host, self.cfg.port);
        loop {
            info!(driver = %self.cfg.name, %addr, "connecting");
            match TcpStream::connect(&addr).await {
                Ok(stream) => {
                    if let Err(e) = session(&self.cfg, stream, &tx).await {
                        error!(driver = %self.cfg.name, "session: {:#}", e);
                    }
                }
                Err(e) => warn!(driver = %self.cfg.name, "connect: {e}"),
            }
            if tx.is_closed() { break; }
            sleep(Duration::from_secs(self.cfg.reconnect_secs)).await;
        }
    }
}

// ── 会话状态 ──────────────────────────────────────────────────────────────

#[derive(PartialEq, Eq)]
enum State { Connecting, Started }

#[derive(Default)]
struct Seq { vs: u16, vr: u16, unacked_recv: u16 }

impl Seq {
    fn next_vs(&mut self) -> u16 { let s = self.vs; self.vs = (self.vs + 1) % 32768; s }
    fn inc_vr(&mut self) { self.vr = (self.vr + 1) % 32768; }
}

async fn session(
    cfg: &Iec104Config,
    mut stream: TcpStream,
    tx: &mpsc::Sender<Batch>,
) -> anyhow::Result<()> {
    stream.set_nodelay(true)?;
    let mut seq   = Seq::default();
    let mut state = State::Connecting;
    let mut buf   = BytesMut::with_capacity(4096);
    let mut last_recv = Instant::now();
    let mut testfr_at: Option<Instant> = None;

    write(&mut stream, Frame::U(UType::StartDtAct)).await?;
    info!(driver = %cfg.name, "STARTDT_ACT sent");

    loop {
        // t3 空闲检测
        if last_recv.elapsed() >= cfg.t3() && testfr_at.is_none() {
            write(&mut stream, Frame::U(UType::TestFrAct)).await?;
            testfr_at = Some(Instant::now());
        }
        if testfr_at.map_or(false, |t| t.elapsed() >= cfg.t1()) {
            anyhow::bail!("TESTFR timeout");
        }
        // 接收窗口满时主动发 S 帧
        if seq.unacked_recv >= cfg.w {
            write(&mut stream, Frame::S { recv_sn: seq.vr }).await?;
            seq.unacked_recv = 0;
        }

        // 读
        let mut tmp = [0u8; 4096];
        match timeout(Duration::from_millis(100), stream.read(&mut tmp)).await {
            Ok(Ok(0)) => anyhow::bail!("remote closed"),
            Ok(Ok(n)) => { buf.extend_from_slice(&tmp[..n]); last_recv = Instant::now(); testfr_at = None; }
            Ok(Err(e)) => return Err(e.into()),
            Err(_) => {}
        }

        // 解帧
        loop {
            match Frame::decode(&buf) {
                Ok(Some((frame, n))) => {
                    let _ = buf.split_to(n);
                    dispatch(cfg, &frame, &mut stream, &mut seq, &mut state, tx).await?;
                }
                Ok(None) => break,
                Err(e) => { warn!(driver=%cfg.name, "decode: {e}"); buf.clear(); break; }
            }
        }
    }
}

async fn dispatch(
    cfg: &Iec104Config,
    frame: &Frame,
    stream: &mut TcpStream,
    seq: &mut Seq,
    state: &mut State,
    tx: &mpsc::Sender<Batch>,
) -> anyhow::Result<()> {
    match frame {
        Frame::U(UType::StartDtCon) => {
            info!(driver = %cfg.name, "session started");
            *state = State::Started;
            if cfg.auto_interrogate {
                let apdu = asdu::interrogation_cmd(cfg.ca, cfg.qoi);
                let sn = seq.next_vs();
                write(stream, Frame::I { send_sn: sn, recv_sn: seq.vr, apdu }).await?;
            }
        }
        Frame::U(UType::TestFrAct) => write(stream, Frame::U(UType::TestFrCon)).await?,
        Frame::U(UType::TestFrCon) => {}
        Frame::S { .. } => {}
        Frame::I { send_sn, recv_sn: _, apdu } => {
            if *state != State::Started { return Ok(()); }
            if *send_sn != seq.vr {
                warn!(driver=%cfg.name, expected=seq.vr, got=send_sn, "seq mismatch");
            }
            seq.inc_vr();
            seq.unacked_recv += 1;

            let batch = asdu::parse(&cfg.name, cfg.ca, apdu)?;
            if !batch.is_empty() {
                debug!(driver=%cfg.name, n=batch.len(), "-> channel");
                tx.send(batch).await
                    .map_err(|_| anyhow::anyhow!("sink channel closed"))?;
            }

            if seq.unacked_recv >= cfg.w {
                write(stream, Frame::S { recv_sn: seq.vr }).await?;
                seq.unacked_recv = 0;
            }
        }
        _ => {}
    }
    Ok(())
}

async fn write(stream: &mut TcpStream, frame: Frame) -> anyhow::Result<()> {
    stream.write_all(&frame.encode()).await?;
    Ok(())
}
