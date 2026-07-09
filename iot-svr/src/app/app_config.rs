use crate::app::iot_config::IotConfig;
use robotech::mq::mqtt::MqttConfig;
use robotech::tsdb::influxdb::InfluxdbConfig;
use serde::Deserialize;

/// 配置文件结构
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct AppConfig {
    #[serde(default)]
    pub iot: IotConfig,
    #[serde()]
    pub mqtt: MqttConfig,
    #[serde()]
    pub influxdb: InfluxdbConfig,
}
