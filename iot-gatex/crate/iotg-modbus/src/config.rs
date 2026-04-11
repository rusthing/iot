use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModbusConfig {
    pub name: String,
    /// TCP 模式：设备 IP
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    /// Modbus Unit ID / Slave ID
    #[serde(default = "default_unit")]
    pub unit_id: u8,
    /// 轮询间隔（毫秒）
    #[serde(default = "default_interval_ms")]
    pub interval_ms: u64,
    #[serde(default = "default_reconnect")]
    pub reconnect_secs: u64,
    /// 需要轮询的寄存器段
    #[serde(default)]
    pub polls: Vec<PollBlock>,
}

/// 一段连续寄存器轮询定义
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PollBlock {
    /// "coil" | "discrete" | "holding" | "input"
    pub kind: String,
    /// 起始地址（0-based）
    pub start: u16,
    /// 寄存器数量
    pub count: u16,
    /// 可选：tag 前缀，默认用 kind 首字母 + start
    #[serde(default)]
    pub tag_prefix: Option<String>,
}

fn default_port() -> u16 { 502 }
fn default_unit() -> u8 { 1 }
fn default_interval_ms() -> u64 { 1000 }
fn default_reconnect() -> u64 { 5 }
