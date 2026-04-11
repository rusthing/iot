use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::model::Batch;

/// 所有采集驱动实现此 trait。
///
/// 实现者负责：
/// - 建立与设备的连接（TCP / 串口 / UDP …）
/// - 按协议规范收发帧
/// - 将解析结果转为 `Batch` 发送到 `tx`
/// - 连接断开时自动重连，永不退出
#[async_trait]
pub trait Driver: Send + 'static {
    /// 驱动类型名，仅用于日志
    fn protocol(&self) -> &'static str;

    /// 驱动实例名（来自配置）
    fn name(&self) -> &str;

    /// 启动采集循环，直到 `tx` 关闭才退出
    async fn run(self: Box<Self>, tx: mpsc::Sender<Batch>);
}
