use std::time::Duration;

use async_trait::async_trait;
use iotg_core::{Batch, Driver};
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::config::ModbusConfig;

pub struct ModbusDriver {
    pub cfg: ModbusConfig,
}

impl ModbusDriver {
    pub fn new(cfg: ModbusConfig) -> Self { Self { cfg } }
}

#[async_trait]
impl Driver for ModbusDriver {
    fn protocol(&self) -> &'static str { "modbus-tcp" }
    fn name(&self) -> &str { &self.cfg.name }

    async fn run(self: Box<Self>, tx: mpsc::Sender<Batch>) {
        info!(driver = %self.cfg.name, "modbus driver started (stub)");
        warn!(driver = %self.cfg.name, "TODO: implement tokio-modbus polling loop");

        // 骨架：循环等待，直到 channel 关闭
        // 实现步骤：
        //   1. TcpStream::connect(host:port)
        //   2. 按 cfg.polls 逐段 Read Holding Registers
        //   3. 解析响应，构造 DataPoint，tx.send(batch).await
        //   4. sleep(interval_ms)，断连后重连
        loop {
            if tx.is_closed() { break; }
            tokio::time::sleep(Duration::from_secs(self.cfg.reconnect_secs)).await;
        }
    }
}
