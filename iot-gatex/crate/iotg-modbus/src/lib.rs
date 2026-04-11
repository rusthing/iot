//! Modbus TCP/RTU 采集驱动骨架
//!
//! 待实现：
//!   1. 引入 `tokio-modbus` crate（TCP client）或自行实现 RTU over 串口
//!   2. 根据 `polls` 配置循环发 Read Holding/Input Registers 请求
//!   3. 将寄存器值映射为 DataPoint（tag = "hr{addr}" 或别名）
//!   4. 处理 Exception Response，写入 Quality::BAD

pub mod config;
pub mod driver;

pub use config::ModbusConfig;
pub use driver::ModbusDriver;
