use serde::Deserialize;
use std::time::Duration;
use wheel_rs::serde::duration_serde;

/// # 数据接入配置
///
/// ## 字段
/// * `channel_capacity` - 缓存 DataPoint 通道的容量
/// * `flush_interval` - 缓存批量写入 InfluxDB 的间隔
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct IotConfig {
    /// 缓存 DataPoint 通道的容量
    #[serde(default = "default_channel_capacity")]
    pub channel_capacity: usize,
    /// 缓存刷新批量将 DataPoint 写入 InfluxDB 间隔
    #[serde(with = "duration_serde", default = "default_flush_interval")]
    pub flush_interval: Duration,
}

impl Default for IotConfig {
    fn default() -> Self {
        Self {
            channel_capacity: default_channel_capacity(),
            flush_interval: default_flush_interval(),
        }
    }
}

/// 默认通道容量
fn default_channel_capacity() -> usize {
    2 * 1024 * 1024
}

/// 默认刷新间隔：5 秒
fn default_flush_interval() -> Duration {
    Duration::from_secs(5)
}
