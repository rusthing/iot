use async_trait::async_trait;
use iotg_core::{Batch, Driver};
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::config::Hj212Config;

pub struct Hj212Driver {
    pub cfg: Hj212Config,
}

impl Hj212Driver {
    pub fn new(cfg: Hj212Config) -> Self { Self { cfg } }
}

#[async_trait]
impl Driver for Hj212Driver {
    fn protocol(&self) -> &'static str { "hj212" }
    fn name(&self) -> &str { &self.cfg.name }

    async fn run(self: Box<Self>, tx: mpsc::Sender<Batch>) {
        let addr = format!("{}:{}", self.cfg.listen_host, self.cfg.listen_port);
        info!(driver = %self.cfg.name, %addr, "hj212 driver started (stub — TCP server)");
        warn!(driver = %self.cfg.name, "TODO: implement HJ212 TCP server + frame parser");

        // 实现步骤：
        //   1. TcpListener::bind(addr).await
        //   2. loop { let (socket, peer) = listener.accept().await; }
        //   3. 每条连接 spawn 一个 task：
        //        a. 按行读（##...CRLF 作为帧边界）
        //        b. 验 CRC16（CCITT）
        //        c. 解析数据段：QN= ST= CN= MN= CP= datatime= 等字段
        //        d. CN=2011/2031 → 实时/分钟数据，构造 DataPoint
        //           device_id = MN 字段，tag = 污染物编码（如 "a34004"）
        //           value = Rtd 值，quality 由 Flag 字段决定（N=正常）
        //        e. 发送 ACK（CN=9011）
        //        f. tx.send(batch).await
        //
        // max_connections 可用 Arc<Semaphore> 控制并发
        loop {
            if tx.is_closed() { break; }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    }
}
