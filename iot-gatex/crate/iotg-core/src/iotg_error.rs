//! # 命令行错误类型
//!
//! 定义了执行外部命令时可能发生的错误类型。

use std::time::SystemTimeError;

/// # iot-gatex错误枚举
///
/// 该枚举包含了执行外部命令时可能遇到的各种错误类型。
/// 使用 thiserror crate 提供错误信息的自动实现。
#[derive(Debug, thiserror::Error)]
pub enum IotgError {
    #[error("系统时钟错误: {0}")]
    SystemTime(#[from] SystemTimeError),

    #[error("解析错误: {0}")]
    Parse(String),
}
