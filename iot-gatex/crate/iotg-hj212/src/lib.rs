//! HJ212-2017 环保数采仪驱动骨架（TCP 主动上报接收端）
//!
//! HJ212 与其他协议方向相反：网关作为 TCP Server 被动监听，
//! 环保数采仪主动连接并周期推送数据帧。
//!
//! 待实现：
//!   1. TcpListener::bind(listen_addr) 并 accept 连接
//!   2. 按 HJ212 帧格式解析：##{数据段长度}{数据段}CRC{CR}{LF}
//!   3. 数据段 key=value 对解析（如 "a34004-Rtd=52.36,a34004-Flag=N"）
//!   4. 按站点编号（MN）分 device_id，每个数据项为一个 DataPoint
//!   5. 发送 ACK 应答 QN 字段

pub mod config;
pub mod driver;

pub use config::Hj212Config;
pub use driver::Hj212Driver;
