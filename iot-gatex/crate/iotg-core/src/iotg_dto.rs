//! # 数据点类型（复用 `iot-api`）
//!
//! 线协议类型统一定义在 [`iot_api::dto::iot_mq_dto`]，此处仅做转发，
//! 避免网关与服务端各维护一份定义。

pub use iot_api::dto::iot_mq_dto::*;
