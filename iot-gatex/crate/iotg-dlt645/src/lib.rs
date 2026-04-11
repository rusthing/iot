//! DLT645 电表采集驱动骨架（RS485 串口 / TCP 透传）
//!
//! 待实现：
//!   1. 引入 `tokio-serial` crate 或 TCP 透传 socket
//!   2. 按 DLT645-2007 格式构建读数据命令帧（0x11）
//!   3. 解析应答帧：起始 0x68、地址域、控制码、数据长度、数据域（BCD/整型）
//!   4. 将数据标识（如 0x00010000=正向有功电能）映射为 tag 发布

pub mod config;
pub mod driver;

pub use config::Dlt645Config;
pub use driver::Dlt645Driver;
