use robotech::mq::mqtt::MqttConfig;
use robotech::tsdb::influxdb2::Influxdb2Config;
use serde::Deserialize;

/// 配置文件结构
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct AppConfig {
    #[serde()]
    pub mqtt: MqttConfig,
    #[serde()]
    pub influxdb2: Influxdb2Config,
}
