use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct IotMqDto {
    /// 设备标识
    pub device: String,
    /// 指标
    pub metric: String,
    /// 值
    pub value: Value,
    /// 创建时间戳
    pub ts: u64,
}
