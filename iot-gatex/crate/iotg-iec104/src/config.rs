use serde::{Deserialize, Serialize};
use std::time::Duration;
use wheel_rs::serde::duration_serde;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Iec104Config {
    pub name: String,
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub ca: u16,
    /// 断开后重连的间隔
    #[serde(with = "duration_serde", default = "default_reconnect")]
    pub reconnect_interval: Duration,
    #[serde(default = "default_true")]
    pub auto_interrogate: bool,
    #[serde(default = "default_qoi")]
    pub qoi: u8,
    /// 最大未确认 I 帧数（发送窗口）
    #[serde(default = "default_k")]
    pub k: u16,
    /// 最大未确认接收 I 帧数（接收窗口）
    #[serde(default = "default_w")]
    pub w: u16,
    /// 发送或测试 APDU 的超时时间
    #[serde(with = "duration_serde", default = "default_t1")]
    pub t1: Duration,
    /// 无数据报文时发送 S 帧确认的超时
    #[serde(with = "duration_serde", default = "default_t2")]
    pub t2: Duration,
    /// 无数据传输时发送测试帧的超时
    #[serde(with = "duration_serde", default = "default_t3")]
    pub t3: Duration,
}

fn default_port() -> u16 {
    2404
}
fn default_reconnect() -> Duration {
    Duration::from_secs(5)
}
fn default_true() -> bool {
    true
}
fn default_qoi() -> u8 {
    20
}
fn default_k() -> u16 {
    12
}
fn default_w() -> u16 {
    8
}
fn default_t1() -> Duration {
    Duration::from_secs(15)
}
fn default_t2() -> Duration {
    Duration::from_secs(10)
}
fn default_t3() -> Duration {
    Duration::from_secs(20)
}
