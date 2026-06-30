use std::pin::Pin;
use std::time::Duration;

use crate::protocol::apci::IType;
use crate::protocol::asdu::{gi_cmd, kwh_cmd};
use crate::protocol::window::{RecvWindow, SendWindow};
use crate::{
    config::Iec104Config,
    protocol::{
        apci::{Frame, UType},
        asdu,
    },
};
use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use iotg_core::{Batch, Driver};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::time::{interval, sleep_until, Instant, Sleep};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::mpsc,
    time::{sleep, timeout},
};
use tracing::{debug, error, info, warn};

pub struct Iec104Driver {
    pub cfg: Iec104Config,
}

impl Iec104Driver {
    pub fn new(cfg: Iec104Config) -> Self {
        Self { cfg }
    }
}

#[async_trait]
impl Driver for Iec104Driver {
    fn protocol(&self) -> &'static str {
        "iec104"
    }
    fn name(&self) -> &str {
        &self.cfg.name
    }

    async fn run(self: Box<Self>, mq_sender: mpsc::Sender<Batch>) {
        let addr = format!("{}:{}", self.cfg.host, self.cfg.port);
        loop {
            info!(device = %self.cfg.name, %addr, "connecting");
            match timeout(self.cfg.t0, TcpStream::connect(&addr)).await {
                Ok(Ok(tcp_stream)) => {
                    let mut session = Session::new(self.cfg.clone(), mq_sender.clone());
                    if let Err(e) = session.start(tcp_stream).await {
                        error!(device = %self.cfg.name, "session: {:#}", e);
                    }
                }
                Ok(Err(e)) => warn!(device = %self.cfg.name, "connect: {e}"),
                Err(_) => warn!(device = %self.cfg.name, "connect timeout"),
            }
            if mq_sender.is_closed() {
                break;
            }
            sleep(self.cfg.reconnect_interval).await;
        }
    }
}

struct Session {
    cfg: Iec104Config,
    mq_tx: mpsc::Sender<Batch>,
}

impl Session {
    fn new(cfg: Iec104Config, mq_tx: mpsc::Sender<Batch>) -> Self {
        Self { cfg, mq_tx }
    }

    async fn start(&mut self, stream: TcpStream) -> anyhow::Result<()> {
        // 禁用 Nagle 算法（Nagle's Algorithm），TCP 默认会缓存小数据包，等待累积到一定量后再一起发送
        stream.set_nodelay(true)?;
        // 分开读写半流
        let (mut reader, mut writer) = stream.into_split();

        // 写帧的消息通道
        let (u_frame_writer_sender, mut u_frame_writer_receiver) = mpsc::channel::<UType>(128);
        let (i_frame_writer_sender, mut i_frame_writer_receiver) = mpsc::channel::<Bytes>(128);
        let (s_frame_writer_sender, mut s_frame_writer_receiver) = mpsc::channel::<()>(128);

        // 初始化发送窗口和接收窗口
        let mut i_frame_send_window = SendWindow::new(self.cfg.k);
        let mut i_frame_recv_window = RecvWindow::new(self.cfg.w);

        let mut bytes = BytesMut::with_capacity(4096);
        let mut buffer = [0u8; 4096];

        // 创建 t1/t2/t3 的 Sleep 对象
        let mut t1_active = false;
        let mut t2_active = false;
        // t3 始终 active，用 sleep 表示
        let t1_sleep = sleep_until(Instant::now() + Duration::from_secs(100000));
        let t2_sleep = sleep_until(Instant::now() + Duration::from_secs(100000));
        let t3_sleep = sleep_until(Instant::now() + self.cfg.t3);
        // 把局部变量放到栈上的固定位置，否则在select!中无法保证其内存位置不变
        tokio::pin!(t1_sleep, t2_sleep, t3_sleep);

        // 是否等待U帧确认
        let mut is_wait_u_frame_confirm = false;

        // 发送 STARTDT_ACT 指令
        u_frame_writer_sender.send(UType::StartDtAct).await?;

        // 创建定时器定时发送总召唤指令
        let get_gi_interval = self.cfg.get_gi_interval;
        let mut gi_ticker = interval(get_gi_interval);
        let qoi = self.cfg.clone().qoi;
        let i_frame_writer_sender_clone = i_frame_writer_sender.clone();
        let driver_name_clone = self.cfg.name.clone();
        tokio::spawn(async move {
            loop {
                gi_ticker.tick().await;
                let asdu = gi_cmd(qoi);
                if let Err(e) = i_frame_writer_sender_clone.send(asdu).await {
                    error!(driver = %driver_name_clone, "send i_frame general interrogation error: {:#}", e);
                    break;
                }
            }
        });

        // 创建定时器定时发送召唤电度指令
        let get_kwh_interval = self.cfg.get_kwh_interval;
        let mut kwh_ticker = interval(get_kwh_interval);
        let qcc = self.cfg.clone().qcc;
        let i_frame_writer_sender_clone = i_frame_writer_sender.clone();
        let driver_name_clone = self.cfg.name.clone();
        tokio::spawn(async move {
            loop {
                kwh_ticker.tick().await;
                let asdu = kwh_cmd(qcc);
                if let Err(e) = i_frame_writer_sender_clone.send(asdu).await {
                    error!(driver = %driver_name_clone, "send i_frame kwh interrogation error: {:#}", e);
                    break;
                }
            }
        });

        loop {
            tokio::select! {
                // 读取要写的U帧
                msg = u_frame_writer_receiver.recv(), if is_wait_u_frame_confirm == false => {
                    if let Some(utype) = msg {
                        if let Err(e) = write(&mut writer, &self.cfg.name, Frame::U(utype)).await {
                            anyhow::bail!("write u_frame error: {e}");
                        }
                        // 如果是U帧指令
                        if let UType::StartDtAct | UType::StopDtAct | UType::TestFrAct = utype {
                            // 等待U帧确认
                            is_wait_u_frame_confirm = true;
                            // 重置并激活 t1
                            t1_sleep.as_mut().reset(Instant::now() + self.cfg.t1);
                            t1_active = true;
                        }
                        // 重置 t3
                        t3_sleep.as_mut().reset(Instant::now() + self.cfg.t3);
                    } else {
                        anyhow::bail!("receive write u_frame msg is None");
                    }
                }
                // 读取要写的I帧
                msg = i_frame_writer_receiver.recv(), if is_wait_u_frame_confirm == false
                        && !i_frame_send_window.window.is_full() => {
                    if let Some(asdu) = msg {
                        // 设置I帧的发送序列号和期待要接收的序列号
                        let i_type = IType{
                            ns: i_frame_send_window.current(),
                            nr: i_frame_recv_window.current(),
                            asdu,
                        };
                        if let Err(e) = write(&mut writer, &self.cfg.name, Frame::I(i_type)).await {
                            anyhow::bail!("write i_frame error: {e}");
                        }
                        // 发送窗口递增
                        i_frame_send_window.window.inc();
                        // 重置 t1
                        t1_sleep.as_mut().reset(Instant::now() + self.cfg.t1);
                        t1_active = true;
                        // 重置 t3
                        t3_sleep.as_mut().reset(Instant::now() + self.cfg.t3);
                    } else {
                        anyhow::bail!("receive write i_frame msg is None");
                    }
                }
                // 读取要写的S帧
                msg = s_frame_writer_receiver.recv() => {
                    if msg.is_none() {
                        anyhow::bail!("receive write s_frame msg is None");
                    }
                    let nr=i_frame_recv_window.current();
                    if let Err(e) = write(&mut writer, &self.cfg.name, Frame::S { nr }).await {
                        anyhow::bail!("write s_frame error: {e}");
                    }
                    // 己方未应答接收I帧的帧数清零
                    i_frame_recv_window.clear();
                    // 重置 t3
                    t3_sleep.as_mut().reset(Instant::now() + self.cfg.t3);
                }
                // 读取网络数据
                n = reader.read(&mut buffer) => {
                    match n {
                        Ok(0) => {
                            warn!(driver=%self.cfg.name, "read 0");
                            anyhow::bail!("read 0");
                        }
                        Ok(n) => {
                            bytes.extend_from_slice(&buffer[..n]);
                        }
                        Err(e) => {
                            warn!(driver=%self.cfg.name, "read: {e}");
                            anyhow::bail!("read error: {e}");
                        }
                    }
                    // 解帧
                    loop {
                        match Frame::decode(&bytes) {
                            Ok(Some((frame, n))) => {
                                let _ = bytes.split_to(n);
                                self.dispatch(
                                    &frame,
                                    &mut i_frame_send_window,
                                    &mut i_frame_recv_window,
                                    &u_frame_writer_sender,
                                    &i_frame_writer_sender,
                                    &s_frame_writer_sender,
                                    &mut t1_active,
                                    &mut t2_sleep,
                                    &mut t2_active,
                                    &mut is_wait_u_frame_confirm,
                                ).await?;
                            }
                            Ok(None) => break,
                            Err(e) => anyhow::bail!("decode frame error: {e}")
                        }
                    }
                    // 重置 t3
                    t3_sleep.as_mut().reset(Instant::now() + self.cfg.t3);
                }
                // t1 超时（断开连接）
                _ = &mut t1_sleep, if t1_active => {
                    anyhow::bail!("t1 timeout");
                }
                // t2 超时（发 S 帧）
                _ = &mut t2_sleep, if t2_active => {
                    s_frame_writer_sender.send(()).await?;
                    t2_active = false;
                }
                // t3 超时（发 TESTFR act）
                _ = &mut t3_sleep => {
                    u_frame_writer_sender.send(UType::TestFrAct).await?;
                }
            }
        }
    }

    async fn dispatch(
        &self,
        frame: &Frame,
        send_window: &mut SendWindow,
        recv_window: &mut RecvWindow,
        u_frame_writer_sender: &mpsc::Sender<UType>,
        i_frame_writer_sender: &mpsc::Sender<Bytes>,
        s_frame_writer_sender: &mpsc::Sender<()>,
        t1_active: &mut bool,
        t2_sleep: &mut Pin<&mut Sleep>,
        t2_active: &mut bool,
        is_wait_u_frame_confirm: &mut bool,
    ) -> anyhow::Result<()> {
        debug!(driver = %self.cfg.name, "dispatch frame: {:?}", frame);
        match frame {
            Frame::U(UType::StartDtAct) => {
                u_frame_writer_sender.send(UType::StartDtCon).await?;
            }
            Frame::U(UType::StartDtCon) => {
                info!(driver = %self.cfg.name, "session created");
                *is_wait_u_frame_confirm = false;
                let asdu = gi_cmd(self.cfg.qoi);
                i_frame_writer_sender.send(asdu).await?;
            }
            Frame::U(UType::StopDtAct) => u_frame_writer_sender.send(UType::StopDtCon).await?,
            Frame::U(UType::StopDtCon) => {
                *is_wait_u_frame_confirm = false;
            }
            Frame::U(UType::TestFrAct) => u_frame_writer_sender.send(UType::TestFrCon).await?,
            Frame::U(UType::TestFrCon) => {
                *is_wait_u_frame_confirm = false;
            }
            Frame::S { nr } => {
                // 发送窗口确认
                send_window.confirm(*nr);
                // 如果发送窗口为空，取消 t1 计时
                if send_window.window.is_empty() {
                    *t1_active = false;
                }
            } // 发送窗口确认
            Frame::I(i_type) => {
                let ns = i_type.ns;
                let nr = i_type.nr;
                // 发送窗口确认
                send_window.confirm(nr);
                // 如果发送窗口为空，取消 t1 计时
                if send_window.window.is_empty() {
                    *t1_active = false;
                }
                // 判断接收窗口是否符合预期
                if ns != recv_window.current() {
                    anyhow::bail!(
                        "received I frame ns {} != expected {}",
                        ns,
                        recv_window.current()
                    );
                }
                // 接收窗口递增
                recv_window.window.inc();
                // 己方未应答接收I帧的帧数超过 w，发 S 帧
                if recv_window.window.is_full() {
                    s_frame_writer_sender.send(()).await?;
                } else {
                    // 否则重置 t2 计时
                    t2_sleep.as_mut().reset(Instant::now() + self.cfg.t2);
                    *t2_active = true;
                }

                let batch = asdu::parse(
                    &self.cfg.name,
                    &self.cfg.ca_prefix,
                    &self.cfg.ioa_prefix,
                    &i_type.asdu,
                )?;
                if !batch.is_empty() {
                    debug!(driver=%self.cfg.name, n=batch.len(), "-> channel");
                    self.mq_tx
                        .send(batch)
                        .await
                        .map_err(|_| anyhow::anyhow!("sink channel closed"))?;
                }
            }
        }
        Ok(())
    }
}

// async fn session(
//     cfg: &Iec104Config,
//     mut stream: TcpStream,
//     tx: &mpsc::Sender<Batch>,
// ) -> anyhow::Result<()> {
//     // 禁用 Nagle 算法（Nagle's Algorithm），TCP 默认会缓存小数据包，等待累积到一定量后再一起发送
//     stream.set_nodelay(true)?;
//     // 分开读写半流
//     let (read_half, write_half) = stream.into_split();
//
//     let mut seq = Seq::default();
//     let mut state = State::Connecting;
//     let mut buf = BytesMut::with_capacity(4096);
//     let mut last_recv = Instant::now();
//     let mut testfr_at: Option<Instant> = None;
//
//     write(&mut stream, Frame::U(UType::StartDtAct)).await?;
//     info!(driver = %cfg.name, "STARTDT_ACT sent");
//
//     loop {
//         let mut buffer = [0u8; 4096];
//         tokio::select! {
//             read_result = stream.read(&mut buffer) => {
//                 handle_read_result(read_result).await?;
//             },
//         }
//
//         // t3 空闲检测
//         if last_recv.elapsed() >= cfg.t3 && testfr_at.is_none() {
//             write(&mut stream, Frame::U(UType::TestFrAct)).await?;
//             testfr_at = Some(Instant::now());
//         }
//         if testfr_at.map_or(false, |t| t.elapsed() >= cfg.t1) {
//             anyhow::bail!("TESTFR timeout");
//         }
//         // 接收窗口满时主动发 S 帧
//         if seq.unacked_recv >= cfg.w {
//             write(&mut stream, Frame::S { nr: seq.nr }).await?;
//             seq.unacked_recv = 0;
//         }
//
//         // 读
//         let mut tmp = [0u8; 4096];
//         match timeout(Duration::from_millis(100), stream.read(&mut tmp)).await {
//             Ok(Ok(0)) => anyhow::bail!("remote closed"),
//             Ok(Ok(n)) => {
//                 buf.extend_from_slice(&tmp[..n]);
//                 last_recv = Instant::now();
//                 testfr_at = None;
//             }
//             Ok(Err(e)) => return Err(e.into()),
//             Err(_) => {}
//         }
//
//         // 解帧
//         loop {
//             match Frame::decode(&buf) {
//                 Ok(Some((frame, n))) => {
//                     let _ = buf.split_to(n);
//                     dispatch(cfg, &frame, &mut stream, &mut seq, &mut state, tx).await?;
//                 }
//                 Ok(None) => break,
//                 Err(e) => {
//                     warn!(driver=%cfg.name, "decode: {e}");
//                     buf.clear();
//                     break;
//                 }
//             }
//         }
//     }
// }
//
// async fn handle_read_result(read_result: Read<TcpStream>) -> anyhow::Result<()> {
//     match read_result {
//         Ok(0) => {
//             warn!(driver=%cfg.name, "read 0");
//             anyhow::bail!("read 0");
//         }
//         Ok(n) => {
//             buf.extend_from_slice(&buffer[..n]);
//             last_recv = Instant::now();
//             testfr_at = None;
//         }
//         Err(e) => {
//             warn!(driver=%cfg.name, "read: {e}");
//             anyhow::bail!("read error: {e}");
//         }
//     };
//     // 解帧
//     loop {
//         match Frame::decode(&buf) {
//             Ok(Some((frame, n))) => {
//                 let _ = buf.split_to(n);
//                 dispatch(cfg, &frame, &mut stream, &mut seq, &mut state, tx).await?;
//             }
//             Ok(None) => break,
//             Err(e) => {
//                 warn!(driver=%cfg.name, "decode: {e}");
//                 buf.clear();
//                 break;
//             }
//         }
//     }
// }
//
// async fn dispatch(
//     cfg: &Iec104Config,
//     frame: &Frame,
//     seq: &mut Seq,
//     state: &mut State,
// ) -> anyhow::Result<()> {
//     match frame {
//         Frame::U(UType::StartDtCon) => {
//             info!(driver = %cfg.name, "session started");
//             *state = State::Started;
//             let apdu = asdu::gi_cmd(cfg.qoi);
//             let sn = seq.next_ns();
//             write(
//                 stream,
//                 Frame::I {
//                     send_sn: sn,
//                     recv_sn: seq.nr,
//                     apdu,
//                 },
//             )
//             .await?;
//         }
//         Frame::U(UType::TestFrAct) => write(stream, Frame::U(UType::TestFrCon)).await?,
//         Frame::U(UType::TestFrCon) => {}
//         Frame::S { .. } => {}
//         Frame::I {
//             send_sn,
//             recv_sn: _,
//             apdu,
//         } => {
//             if *state != State::Started {
//                 return Ok(());
//             }
//             if *send_sn != seq.nr {
//                 warn!(driver=%cfg.name, expected=seq.nr, got=send_sn, "seq mismatch");
//             }
//             seq.set_nr();
//             seq.unacked_recv += 1;
//
//             let batch = asdu::parse(&cfg.name, &cfg.ca_prefix, &cfg.ioa_prefix, apdu)?;
//             if !batch.is_empty() {
//                 debug!(driver=%cfg.name, n=batch.len(), "-> channel");
//                 tx.send(batch)
//                     .await
//                     .map_err(|_| anyhow::anyhow!("sink channel closed"))?;
//             }
//
//             if seq.unacked_recv >= cfg.w {
//                 write(stream, Frame::S { nr: seq.nr }).await?;
//                 seq.unacked_recv = 0;
//             }
//         }
//         _ => {}
//     }
//     Ok(())
// }
//
async fn write(writer: &mut OwnedWriteHalf, driver_name: &str, frame: Frame) -> anyhow::Result<()> {
    debug!(driver=%driver_name, "writing frame: {:?}", frame);
    writer.write_all(&frame.encode()).await?;
    Ok(())
}
