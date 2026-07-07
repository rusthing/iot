use serde::{Deserialize, Serialize};
use std::time::Duration;
use wheel_rs::serde::duration_serde;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Iec104Config {
    /// 采集设备名称
    pub name: String,
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    /// 断开后重连的间隔
    #[serde(with = "duration_serde", default = "default_reconnect")]
    pub reconnect_interval: Duration,
    /// 总召唤(General Interrogation)间隔
    #[serde(default = "default_get_gi")]
    pub get_gi: bool,
    /// 总召唤(General Interrogation)间隔
    #[serde(with = "duration_serde", default = "default_get_gi_interval")]
    pub get_gi_interval: Duration,
    /// 召唤电度间隔
    #[serde(default = "default_get_kwh")]
    pub get_kwh: bool,
    /// 召唤电度间隔
    #[serde(with = "duration_serde", default = "default_get_kwh_interval")]
    pub get_kwh_interval: Duration,
    /// # 总召唤限定词，用于指定总召唤的范围和类型
    /// 默认20为全站总召唤
    #[serde(default = "default_qoi")]
    pub qoi: u8,
    /// # 电度召唤限定词，用于指定电度召唤的范围和类型
    /// 默认0x45为全局总召唤
    #[serde(default = "default_qcc")]
    pub qcc: u8,
    /// 最大未确认 I 帧数（发送窗口）
    #[serde(default = "default_k")]
    pub k: usize,
    /// 最大未确认接收 I 帧数（接收窗口）
    #[serde(default = "default_w")]
    pub w: usize,
    /// # TCP 连接的超时时间
    #[serde(with = "duration_serde", default = "default_t0")]
    pub t0: Duration,
    /// # 发送U帧和I帧后等待确认的超时时间
    /// 发送每一个U帧和I帧都要计时，超时后断开重连
    #[serde(with = "duration_serde", default = "default_t1")]
    pub t1: Duration,
    /// # 无数据报文时发送 S 帧确认的超时
    #[serde(with = "duration_serde", default = "default_t2")]
    pub t2: Duration,
    /// # 空闲的超时时间
    /// 如果超时了就发送测试帧
    #[serde(with = "duration_serde", default = "default_t3")]
    pub t3: Duration,
}

fn default_port() -> u16 {
    2404
}
fn default_reconnect() -> Duration {
    Duration::from_secs(5)
}
fn default_get_gi() -> bool {
    true
}
fn default_get_gi_interval() -> Duration {
    Duration::from_mins(15)
}
fn default_get_kwh() -> bool {
    false
}
fn default_get_kwh_interval() -> Duration {
    Duration::from_hours(1)
}
fn default_qoi() -> u8 {
    20
}
fn default_qcc() -> u8 {
    0x45
}
fn default_k() -> usize {
    12
}
fn default_w() -> usize {
    8
}
fn default_t0() -> Duration {
    Duration::from_secs(30)
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
