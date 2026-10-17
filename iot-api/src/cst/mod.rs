//! # 常量定义（CST）
//!
//! IOT 网关与服务端共用的常量。

/// MQTT 消息体（采集数据点）默认 QoS
pub const DEFAULT_MQTT_QOS: u8 = 1;

/// 采集数据点所在 MQTT topic 的默认前缀
pub const DEFAULT_MQTT_TOPIC_PREFIX: &str = "iot-gatex";
