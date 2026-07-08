use serde::{Deserialize, Serialize};
use std::time::Duration;
use wheel_rs::serde::duration_serde;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct MqttConfig {
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_client_id")]
    pub client_id: String,
    /// 主题
    #[serde(default = "default_topic")]
    pub topic: String,
    /// QoS 0 / 1 / 2
    #[serde(default = "default_qos")]
    pub qos: u8,
    /// rumqttc 内部 channel 容量
    #[serde(default = "default_cap")]
    pub channel_capacity: usize,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    /// keep-alive 秒数
    #[serde(default = "default_keepalive")]
    pub keepalive_secs: u64,
    /// rumqttc 内部 channel 容量
    #[serde(with = "duration_serde", default = "default_flush_interval")]
    pub flush_interval: Duration,
}

impl Default for MqttConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: default_port(),
            client_id: default_client_id(),
            topic: default_topic(),
            qos: default_qos(),
            channel_capacity: default_cap(),
            username: None,
            password: None,
            keepalive_secs: default_keepalive(),
            flush_interval: default_flush_interval(),
        }
    }
}

fn default_port() -> u16 {
    1883
}
fn default_client_id() -> String {
    "iot-gatex".into()
}
fn default_topic() -> String {
    "iot-gatex".into()
}
fn default_qos() -> u8 {
    1
}
fn default_cap() -> usize {
    1024
}
fn default_keepalive() -> u64 {
    30
}

fn default_flush_interval() -> Duration {
    Duration::from_secs(5)
}
