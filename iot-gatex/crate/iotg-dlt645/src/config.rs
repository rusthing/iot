use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Dlt645Config {
    pub name: String,
    /// TCP 透传模式下的 IP（串口模式留空，改用 serial_port）
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default = "default_port")]
    pub port: u16,
    /// 串口模式：设备路径，如 "/dev/ttyUSB0"
    #[serde(default)]
    pub serial_port: Option<String>,
    #[serde(default = "default_baud")]
    pub baud_rate: u32,
    /// 电表地址（12位BCD，如 "000000123456"）
    pub meter_addr: String,
    /// 轮询数据标识列表（DLT645-2007 四字节 hex，如 "00010000"）
    pub data_ids: Vec<String>,
    #[serde(default = "default_interval_ms")]
    pub interval_ms: u64,
    #[serde(default = "default_reconnect")]
    pub reconnect_secs: u64,
}

fn default_port() -> u16 { 8000 }
fn default_baud() -> u32 { 9600 }
fn default_interval_ms() -> u64 { 5000 }
fn default_reconnect() -> u64 { 10 }
