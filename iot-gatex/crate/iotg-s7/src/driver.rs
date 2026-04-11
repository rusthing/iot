use std::time::Duration;

use async_trait::async_trait;
use iotg_core::{Batch, Driver};
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::config::S7Config;

pub struct S7Driver {
    pub cfg: S7Config,
}

impl S7Driver {
    pub fn new(cfg: S7Config) -> Self { Self { cfg } }
}

#[async_trait]
impl Driver for S7Driver {
    fn protocol(&self) -> &'static str { "s7" }
    fn name(&self) -> &str { &self.cfg.name }

    async fn run(self: Box<Self>, tx: mpsc::Sender<Batch>) {
        info!(driver = %self.cfg.name, "s7 driver started (stub)");
        warn!(driver = %self.cfg.name, "TODO: implement S7Comm ISO-on-TCP loop");

        // 实现步骤：
        //   1. TcpStream::connect(host:102)
        //   2. 发送 COTP Connection Request（TPKT 包装）
        //   3. 发送 S7 Setup Communication（PDU 协商，最大 PDU 通常 240B）
        //   4. 将 cfg.reads 按 PDU 大小分批，构建 Read Var Request
        //   5. 解析 Read Var Response，按 data_type 转 Value
        //   6. tx.send(batch)；sleep(interval_ms)；断连后重连
        loop {
            if tx.is_closed() { break; }
            tokio::time::sleep(Duration::from_secs(self.cfg.reconnect_secs)).await;
        }
    }
}
