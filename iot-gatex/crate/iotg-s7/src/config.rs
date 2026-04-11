use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct S7Config {
    pub name: String,
    pub host: String,
    /// S7 ISO-on-TCP 端口（默认 102）
    #[serde(default = "default_port")]
    pub port: u16,
    /// PLC 机架号（通常 0）
    #[serde(default)]
    pub rack: u8,
    /// PLC 槽号（S7-300 = 2，S7-1200/1500 = 0 或 1）
    #[serde(default = "default_slot")]
    pub slot: u8,
    /// 轮询间隔（毫秒）
    #[serde(default = "default_interval_ms")]
    pub interval_ms: u64,
    #[serde(default = "default_reconnect")]
    pub reconnect_secs: u64,
    /// 变量读取列表
    pub reads: Vec<S7VarDef>,
}

/// 单个 S7 变量定义
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct S7VarDef {
    /// 发布用的 tag 名，如 "speed_setpoint"
    pub tag: String,
    /// 区域："DB" | "M" | "I" | "Q" | "T" | "C"
    pub area: String,
    /// DB 编号（area="DB" 时有效）
    #[serde(default)]
    pub db_number: u16,
    /// 字节偏移（如 DB1.DBD4 → byte_offset=4）
    pub byte_offset: u32,
    /// 数据类型："BOOL" | "BYTE" | "INT" | "DINT" | "REAL" | "WORD" | "DWORD"
    pub data_type: String,
    /// BOOL 类型时的位偏移（0-7）
    #[serde(default)]
    pub bit_offset: u8,
}

fn default_port() -> u16 { 102 }
fn default_slot() -> u8 { 2 }
fn default_interval_ms() -> u64 { 500 }
fn default_reconnect() -> u64 { 5 }
