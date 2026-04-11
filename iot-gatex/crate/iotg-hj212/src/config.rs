use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Hj212Config {
    pub name: String,
    /// 监听地址，如 "0.0.0.0"
    #[serde(default = "default_listen")]
    pub listen_host: String,
    /// 监听端口（默认 7070）
    #[serde(default = "default_port")]
    pub listen_port: u16,
    /// 最大并发连接数
    #[serde(default = "default_max_conn")]
    pub max_connections: usize,
    /// 是否校验 CRC16
    #[serde(default = "default_true")]
    pub verify_crc: bool,
    /// 是否发送 ACK 回执
    #[serde(default = "default_true")]
    pub send_ack: bool,
}

fn default_listen() -> String { "0.0.0.0".into() }
fn default_port() -> u16 { 7070 }
fn default_max_conn() -> usize { 64 }
fn default_true() -> bool { true }
