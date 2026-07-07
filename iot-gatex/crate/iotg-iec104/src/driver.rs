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
use hex::encode_upper;
use iotg_core::{Batch, Driver};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tokio::time::{interval, sleep_until, Instant, Sleep};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::mpsc,
    time::{sleep, timeout},
};
use tracing::{debug, error, info, warn};

const DRIVER_NAME: &str = "IEC104";

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
            info!(device = %self.name(), %addr, "connecting");
            match timeout(self.cfg.t0, TcpStream::connect(&addr)).await {
                Ok(Ok(tcp_stream)) => {
                    info!(device = %self.name(), %addr, "connected");
                    let mut session = Session::new(self.cfg.clone(), mq_sender.clone());
                    if let Err(e) = session.start(tcp_stream).await {
                        error!(device = %self.name(), "session: {:#}", e);
                    }
                }
                Ok(Err(e)) => warn!(device = %self.name(), "connect: {e}"),
                Err(_) => warn!(device = %self.name(), "connect timeout"),
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
        let device = self.cfg.name.clone();

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

        // 创建任务定时器，用于定时发送召唤指令
        let mut join_set = JoinSet::new();
        let (mut start_task_sender, start_task_receiver) = watch::channel(false); // 初始为 false
        // 创建定时器定时发送总召唤指令
        if self.cfg.get_gi {
            let get_gi_interval = self.cfg.get_gi_interval;
            let mut gi_ticker = interval(get_gi_interval);
            let qoi = self.cfg.clone().qoi;
            let i_frame_writer_sender_clone = i_frame_writer_sender.clone();
            let device_clone = device.clone();
            let mut start_task_receiver_clone = start_task_receiver.clone();
            join_set.spawn(async move {
                // 等待激活
                while !*start_task_receiver_clone.borrow() {
                    if let Err(e) = start_task_receiver_clone.changed().await {
                        error!(device = %device_clone, "start_task_receiver changed error: {:#}", e);
                        return;
                    }
                }
                info!(device = %device_clone, "start execute loop get general interrogation");
                // 循环发送总召唤指令
                loop {
                    gi_ticker.tick().await;
                    let asdu = gi_cmd(qoi);
                    info!(device = %device_clone, "send i_frame general interrogation: {}", encode_upper(&asdu));
                    if let Err(e) = i_frame_writer_sender_clone.send(asdu).await {
                        error!(device = %device_clone, "send i_frame general interrogation error: {:#}", e);
                        break;
                    }
                }
            });
        }
        // 创建定时器定时发送召唤电度指令
        if self.cfg.get_kwh {
            let get_kwh_interval = self.cfg.get_kwh_interval;
            let mut kwh_ticker = interval(get_kwh_interval);
            let qcc = self.cfg.clone().qcc;
            let i_frame_writer_sender_clone = i_frame_writer_sender.clone();
            let device_clone = device.clone();
            let mut start_task_receiver_clone = start_task_receiver.clone();
            let kwh_delay = self.cfg.get_gi_interval / 2;
            join_set.spawn(async move {
                // 等待激活
                while !*start_task_receiver_clone.borrow() {
                    if let Err(e) = start_task_receiver_clone.changed().await {
                        error!(device = %device_clone, "start_task_receiver changed error: {:#}", e);
                        return;
                    }
                }
                info!(device = %device_clone, "start execute loop get kwh interrogation kwh_delay={:?}", kwh_delay);
                // 延迟启动
                sleep(kwh_delay).await;
                // 循环发送召唤电度指令
                loop {
                    kwh_ticker.tick().await;
                    let asdu = kwh_cmd(qcc);
                    info!(device = %device_clone, "send i_frame kwh interrogation: {}", encode_upper(&asdu));
                    if let Err(e) = i_frame_writer_sender_clone.send(asdu).await {
                        error!(device = %device_clone, "send i_frame kwh interrogation error: {:#}", e);
                        break;
                    }
                }
            });
        }

        loop {
            tokio::select! {
                // 读取要写的U帧
                msg = u_frame_writer_receiver.recv(), if is_wait_u_frame_confirm == false => {
                    if let Some(utype) = msg {
                        if let Err(e) = write(&mut writer, &device, Frame::U(utype)).await {
                            anyhow::bail!("write u_frame error: {e}");
                        }
                        // 如果是U帧指令
                        if let UType::StartDtAct | UType::StopDtAct | UType::TestFrAct = utype {
                            // 等待U帧确认
                            is_wait_u_frame_confirm = true;
                            // 重置并激活 t1
                            debug!(device=%device, "开始 t1 计时");
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
                        let i_type = IType {
                            ns: i_frame_send_window.window.current(),
                            nr: i_frame_recv_window.window.current(),
                            asdu,
                        };
                        if let Err(e) = write(&mut writer, &self.cfg.name, Frame::I(i_type)).await {
                            anyhow::bail!("write i_frame error: {e}");
                        }
                        // 发送窗口递增
                        i_frame_send_window.window.inc();
                        // 己方未应答接收I帧的帧数清零
                        i_frame_recv_window.clear();
                        // 重置 t1
                        debug!(device=%device, "开始 t1 计时");
                        t1_sleep.as_mut().reset(Instant::now() + self.cfg.t1);
                        t1_active = true;
                        // 关闭 t2 定时器
                        t2_active = false;
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
                    let nr=i_frame_recv_window.window.current();
                    if let Err(e) = write(&mut writer, &self.cfg.name, Frame::S { nr }).await {
                        anyhow::bail!("write s_frame error: {e}");
                    }
                    // 己方未应答接收I帧的帧数清零
                    i_frame_recv_window.clear();
                    // 关闭 t2 定时器
                    t2_active = false;
                    // 重置 t3
                    t3_sleep.as_mut().reset(Instant::now() + self.cfg.t3);
                }
                // 读取网络数据
                n = reader.read(&mut buffer) => {
                    match n {
                        Ok(0) => {
                            warn!(device=%device, "read 0");
                            anyhow::bail!("read 0");
                        }
                        Ok(n) => {
                            bytes.extend_from_slice(&buffer[..n]);
                        }
                        Err(e) => {
                            warn!(device=%device, "read: {e}");
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
                                    &s_frame_writer_sender,
                                    &mut t1_active,
                                    &mut t2_sleep,
                                    &mut t2_active,
                                    &mut start_task_sender,
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
        i_frame_send_window: &mut SendWindow,
        i_frame_recv_window: &mut RecvWindow,
        u_frame_writer_sender: &mpsc::Sender<UType>,
        s_frame_writer_sender: &mpsc::Sender<()>,
        t1_active: &mut bool,
        t2_sleep: &mut Pin<&mut Sleep>,
        t2_active: &mut bool,
        start_task_sender: &mut watch::Sender<bool>,
        is_wait_u_frame_confirm: &mut bool,
    ) -> anyhow::Result<()> {
        let device = self.cfg.name.clone();
        debug!(device = %device, "dispatch frame: {frame}");
        match frame {
            Frame::U(UType::StartDtAct) => {
                info!(device=%device, "received StartDtAct -> send StartDtCon");
                u_frame_writer_sender.send(UType::StartDtCon).await?;
            }
            Frame::U(UType::StartDtCon) => {
                info!(device=%device, "received StartDtCon -> close t1 timer, cancel wait U frame confirm, session created");
                *t1_active = false;
                *is_wait_u_frame_confirm = false;
                start_task_sender.send(true)?;
            }
            Frame::U(UType::StopDtAct) => {
                info!(device=%device, "received StopDtAct");
                u_frame_writer_sender.send(UType::StopDtCon).await?;
            }
            Frame::U(UType::StopDtCon) => {
                info!(device=%device, "received StopDtCon -> close t1 timer, cancel wait U frame confirm");
                *t1_active = false;
                *is_wait_u_frame_confirm = false;
            }
            Frame::U(UType::TestFrAct) => {
                info!(device=%device, "received TestFrAct -> send TestFrCon");
                u_frame_writer_sender.send(UType::TestFrCon).await?;
            }
            Frame::U(UType::TestFrCon) => {
                info!(device=%device, "received TestFrCon -> close t1 timer, cancel wait U frame confirm");
                *t1_active = false;
                *is_wait_u_frame_confirm = false;
            }
            Frame::S { nr } => {
                info!(device=%device, "received S frame: nr={nr}");
                // 判断是否在发送窗口内
                if !i_frame_send_window.window.is_in(*nr) {
                    anyhow::bail!(
                        "I frame received nr={nr} not in {}",
                        i_frame_send_window.window.bounds(),
                    );
                }
                // 发送窗口确认
                i_frame_send_window.confirm(*nr);
                // 如果发送窗口为空，取消 t1 计时
                if i_frame_send_window.window.is_empty() {
                    debug!(device=%device, "关闭 t1 计时");
                    *t1_active = false;
                }
            }
            Frame::I(i_type) => {
                let ns = i_type.ns;
                let nr = i_type.nr;
                // 判断是否在发送窗口内
                if !i_frame_send_window.window.is_in(nr) {
                    anyhow::bail!(
                        "I frame received nr={nr} not in {}",
                        i_frame_send_window.window.bounds(),
                    );
                }
                // 发送窗口确认
                i_frame_send_window.confirm(nr);
                // 如果发送窗口为空，取消 t1 计时
                if i_frame_send_window.window.is_empty() {
                    debug!(device=%device, "关闭 t1 计时");
                    *t1_active = false;
                }
                // 判断接收窗口是否符合预期
                if ns != i_frame_recv_window.window.current() {
                    anyhow::bail!(
                        "I frame received ns expected {} but {}",
                        ns,
                        i_frame_recv_window.window.current()
                    );
                }
                // 接收窗口递增
                i_frame_recv_window.window.inc();
                // 己方未应答接收I帧的帧数超过 w，发 S 帧
                if i_frame_recv_window.window.is_full() {
                    s_frame_writer_sender.send(()).await?;
                } else {
                    // 否则重置 t2 计时
                    t2_sleep.as_mut().reset(Instant::now() + self.cfg.t2);
                    *t2_active = true;
                }

                let batch = asdu::parse(&DRIVER_NAME, &device, &i_type.asdu)?;
                if !batch.is_empty() {
                    debug!(device=%device, n=batch.len(), "-> channel");
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

async fn write(writer: &mut OwnedWriteHalf, device: &str, frame: Frame) -> anyhow::Result<()> {
    debug!(device=%device, "writing frame: {:?}", frame);
    writer.write_all(&frame.encode()).await?;
    Ok(())
}
