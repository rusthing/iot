use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Iec104Config {
    pub name: String,
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub ca: u16,
    #[serde(default = "default_reconnect")]
    pub reconnect_secs: u64,
    #[serde(default = "default_true")]
    pub auto_interrogate: bool,
    #[serde(default = "default_qoi")]
    pub qoi: u8,
    // 链路参数
    #[serde(default = "default_k")]  pub k: u16,
    #[serde(default = "default_w")]  pub w: u16,
    #[serde(default = "default_t1")] pub t1_secs: u64,
    #[serde(default = "default_t2")] pub t2_secs: u64,
    #[serde(default = "default_t3")] pub t3_secs: u64,
}

impl Iec104Config {
    pub fn t1(&self) -> Duration { Duration::from_secs(self.t1_secs) }
    pub fn t2(&self) -> Duration { Duration::from_secs(self.t2_secs) }
    pub fn t3(&self) -> Duration { Duration::from_secs(self.t3_secs) }
}

fn default_port() -> u16 { 2404 }
fn default_reconnect() -> u64 { 5 }
fn default_true() -> bool { true }
fn default_qoi() -> u8 { 20 }
fn default_k() -> u16 { 12 }
fn default_w() -> u16 { 8 }
fn default_t1() -> u64 { 15 }
fn default_t2() -> u64 { 10 }
fn default_t3() -> u64 { 20 }
