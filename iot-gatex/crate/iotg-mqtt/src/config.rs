use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MqttSinkConfig {
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_client_id")]
    pub client_id: String,
    /// topic 前缀；完整 topic = {prefix}/{driver}/{device_id}/{tag}
    #[serde(default = "default_prefix")]
    pub topic_prefix: String,
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
}

impl Default for MqttSinkConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 1883,
            client_id: "iot-gatex".into(),
            topic_prefix: "gatex".into(),
            qos: 1,
            channel_capacity: 1024,
            username: None,
            password: None,
            keepalive_secs: 30,
        }
    }
}

fn default_port() -> u16 { 1883 }
fn default_client_id() -> String { "iot-gatex".into() }
fn default_prefix() -> String { "gatex".into() }
fn default_qos() -> u8 { 1 }
fn default_cap() -> usize { 1024 }
fn default_keepalive() -> u64 { 30 }
