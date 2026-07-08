use derive_setters::Setters;
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;
use std::fmt::Display;
use typed_builder::TypedBuilder;

/// 所有协议共用的数据点
#[skip_serializing_none]
#[derive(Debug, Serialize, Deserialize, Setters, TypedBuilder)]
#[builder]
pub struct IotMqDto {
    /// 驱动实例名称（来自配置 name 字段）
    pub driver: String,
    /// 设备标识，各协议自定义（如 "ca1"、"unit1"、"slave3"）
    pub device: String,
    /// 指标
    pub metric: String,
    /// 值
    pub value: Value,
    /// 数据质量
    pub quality: Quality,
    /// 采集器接收时间(纳秒级)
    pub ns: u64,
    /// 携带的时标（如有）
    #[builder(default)]
    pub field_ts: Option<u64>,
}

impl Display for IotMqDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} {} {} {:?} {} {:?}",
            self.driver, self.device, self.metric, self.value, self.quality, self.ns, self.field_ts
        )
    }
}

/// 值类型：覆盖所有工业协议常见类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum Value {
    /// 布尔值(单点)
    Bool(bool),
    /// 8位无符号整数(双点[0, 255])
    U8(u8),
    /// 32位无符号整数(32位比特串)
    U32(u32),
    /// 16位整数(归一化/标度化的原始值[-32768, 32767])
    I16(i16),
    /// 32位整数(累积量)
    I32(i32),
    /// 32位浮点（短浮点）
    F32(f32),
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Bool(v) => write!(f, "{}", v),
            Value::U8(v) => write!(f, "{}", v),
            Value::U32(v) => write!(f, "{}", v),
            Value::I16(v) => write!(f, "{}", v),
            Value::I32(v) => write!(f, "{}", v),
            Value::F32(v) => write!(f, "{}", v),
        }
    }
}

// impl From<bool> for Value {
//     fn from(value: bool) -> Self {
//         Self::Bool(value)
//     }
// }
//
// impl From<u8> for Value {
//     fn from(value: u8) -> Self {
//         Self::U8(value)
//     }
// }
//
// impl From<u32> for Value {
//     fn from(value: u32) -> Self {
//         Self::U32(value)
//     }
// }
//
// impl From<i16> for Value {
//     fn from(value: i16) -> Self {
//         Self::I16(value)
//     }
// }
//
// impl From<i32> for Value {
//     fn from(value: i32) -> Self {
//         Self::I32(value)
//     }
// }
//
// impl From<f32> for Value {
//     fn from(value: f32) -> Self {
//         Self::F32(value)
//     }
// }

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
pub type Batch = Vec<IotMqDto>;
