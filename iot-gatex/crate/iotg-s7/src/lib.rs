//! Siemens S7 PLC 采集驱动骨架（S7Comm over ISO-on-TCP, 端口 102）
//!
//! 待实现：
//!   1. 引入 `s7` crate（https://crates.io/crates/s7）或自实现 TPKT/COTP/S7Comm
//!   2. PDU 协商（Setup Communication）
//!   3. 按 cfg.reads 批量构建 Read Var Request（支持 DB/M/I/Q 区域）
//!   4. 解析响应，按数据类型（BOOL/INT/DINT/REAL/WORD）转 Value

pub mod config;
pub mod driver;

pub use config::S7Config;
pub use driver::S7Driver;
