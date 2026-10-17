use crate::config::IotConfig;
use robotech::mq::mqtt::MqttConfig;
use robotech::tsdb::influxdb::InfluxdbConfig;
use serde::Deserialize;

/// # 应用配置
///
/// 对应 `iot-svr.toml`（或 `iot-svr-{profile}.toml`）中的配置内容。
///
/// ## 字段
/// * `iot` - 数据接入与缓存刷新参数
/// * `mqtt` - MQTT 订阅端配置
/// * `influxdb` - InfluxDB 写入配置
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct AppConfig {
    /// 数据接入与缓存刷新参数
    #[serde(default)]
    pub iot: IotConfig,
    /// MQTT 订阅端配置
    pub mqtt: MqttConfig,
    /// InfluxDB 写入配置
    pub influxdb: InfluxdbConfig,
}
