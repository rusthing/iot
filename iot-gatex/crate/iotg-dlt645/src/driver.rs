use std::time::Duration;

use async_trait::async_trait;
use iotg_core::{Batch, Driver};
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::config::Dlt645Config;

pub struct Dlt645Driver {
    pub cfg: Dlt645Config,
}

impl Dlt645Driver {
    pub fn new(cfg: Dlt645Config) -> Self { Self { cfg } }
}

#[async_trait]
impl Driver for Dlt645Driver {
    fn protocol(&self) -> &'static str { "dlt645" }
    fn name(&self) -> &str { &self.cfg.name }

    async fn run(self: Box<Self>, tx: mpsc::Sender<Batch>) {
        info!(driver = %self.cfg.name, "dlt645 driver started (stub)");
        warn!(driver = %self.cfg.name, "TODO: implement DLT645-2007 read loop");

        // 实现步骤（TCP 透传模式）：
        //   1. TcpStream::connect(host:port)  /  串口: SerialStream::open
        //   2. for data_id in cfg.data_ids { 构建请求帧 → write → read 响应 }
        //   3. 校验 CS 累加和，解析 BCD 数据域
        //   4. 构造 DataPoint { device_id = meter_addr, tag = data_id, value }
        //   5. tx.send(batch).await；sleep(interval_ms)；断连后重连
        loop {
            if tx.is_closed() { break; }
            tokio::time::sleep(Duration::from_secs(self.cfg.reconnect_secs)).await;
        }
    }
}
