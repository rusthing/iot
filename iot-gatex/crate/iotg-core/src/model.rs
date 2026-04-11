use chrono::Utc;
use derive_setters::Setters;
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;
use std::time::SystemTimeError;
use typed_builder::TypedBuilder;
use wheel_rs::time_utils::get_current_timestamp;

/// 所有协议共用的数据点
#[skip_serializing_none]
#[derive(Debug, Serialize, Deserialize, Setters, TypedBuilder)]
#[builder]
pub struct DataPoint {
    /// 驱动实例名称（来自配置 name 字段）
    pub driver: String,
    /// 设备标识，各协议自定义（如 "ca1"、"unit1"、"slave3"）
    pub device_id: String,
    /// 数据标签，各协议自定义（如 "ioa1001"、"hr40001"、"meter_kwh"）
    pub tag: String,
    pub value: Value,
    pub quality: Quality,
    /// 本地接收时间
    /// 这里默认值为当前时间戳，懒得考虑系统时间错误的问题
    #[builder(default = Utc::now().timestamp_millis() as u64)]
    pub ts: u64,
    /// 设备携带的时标（如有）
    #[builder(default)]
    pub field_ts: Option<u64>,
}

impl DataPoint {
    /// MQTT topic：{prefix}/{driver}/{device_id}/{tag}
    pub fn mqtt_topic(&self, prefix: &str) -> String {
        format!("{}/{}/{}/{}", prefix, self.driver, self.device_id, self.tag)
    }
}

/// 统一值类型：覆盖所有工业协议常见类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v")]
pub enum Value {
    Bool(bool),
    /// 整数（涵盖计数量、脉冲量、报警码等）
    Int(i64),
    /// 浮点（归一化、标度、短浮点统一转此）
    Float(f64),
    /// 字符串（HJ212 数据项、设备描述等）
    Text(String),
    /// 原始字节（暂不解析的自定义帧）
    Bytes(Vec<u8>),
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}
impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Int(v)
    }
}
impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Value::Int(v as i64)
    }
}
impl From<i16> for Value {
    fn from(v: i16) -> Self {
        Value::Int(v as i64)
    }
}
impl From<u32> for Value {
    fn from(v: u32) -> Self {
        Value::Int(v as i64)
    }
}
impl From<f32> for Value {
    fn from(v: f32) -> Self {
        Value::Float(v as f64)
    }
}
impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Float(v)
    }
}
impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::Text(v)
    }
}

/// 数据品质（协议无关）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Quality {
    /// true = 数据有效且最新
    pub good: bool,
    pub invalid: bool,
    pub not_topical: bool,
    pub substituted: bool,
    pub overflow: bool,
}

impl Quality {
    pub const GOOD: Quality = Quality {
        good: true,
        invalid: false,
        not_topical: false,
        substituted: false,
        overflow: false,
    };
    pub const BAD: Quality = Quality {
        good: false,
        invalid: true,
        not_topical: false,
        substituted: false,
        overflow: false,
    };

    pub fn from_iec104_qds(b: u8) -> Self {
        Self {
            good: (b & 0x80) == 0 && (b & 0x40) == 0,
            invalid: b & 0x80 != 0,
            not_topical: b & 0x40 != 0,
            substituted: b & 0x20 != 0,
            overflow: b & 0x01 != 0,
        }
    }
}

/// 驱动产出的一批数据点（同一帧/轮询周期）
pub type Batch = Vec<DataPoint>;
