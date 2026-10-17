use derive_setters::Setters;
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;
use std::fmt::Display;
use typed_builder::TypedBuilder;

/// # 采集数据点（所有协议共用）
///
/// 网关各协议驱动解析后统一产出的数据点，也是通过 MQTT 发布给服务端的线协议载体。
///
/// ## 字段
/// * `driver` - 驱动实例名称（来自配置 `name` 字段）
/// * `device` - 设备标识，各协议自定义（如 `ca1`、`unit1`、`slave3`）
/// * `metric` - 指标
/// * `value` - 值
/// * `quality` - 数据质量
/// * `ns` - 采集器接收时间（纳秒级）
/// * `field_ms` - 设备携带的时标（毫秒级，如有）
#[skip_serializing_none]
#[derive(Debug, Serialize, Deserialize, Setters, TypedBuilder)]
#[builder]
#[serde(rename_all = "camelCase")]
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
    pub field_ms: Option<u64>,
}

impl Display for IotMqDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} {} {} {:?} {} {:?}",
            self.driver, self.device, self.metric, self.value, self.quality, self.ns, self.field_ms
        )
    }
}

/// # 值类型
///
/// 覆盖所有工业协议常见类型。
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

/// # 数据品质（协议无关）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Quality {
    /// true = 数据有效且最新
    pub good: bool,
    /// 无效
    pub invalid: bool,
    /// 非当前值
    pub not_topical: bool,
    /// 被取代
    pub substituted: bool,
    /// 溢出
    pub overflow: bool,
}

impl Quality {
    /// 有效数据
    pub const GOOD: Quality = Quality {
        good: true,
        invalid: false,
        not_topical: false,
        substituted: false,
        overflow: false,
    };
    /// 无效数据
    pub const BAD: Quality = Quality {
        good: false,
        invalid: true,
        not_topical: false,
        substituted: false,
        overflow: false,
    };

    /// # 由 IEC-104 的 QDS 字节解析数据品质
    ///
    /// ## 参数
    /// * `b` - IEC-104 ASDU 中的品质描述字节
    ///
    /// ## 返回值
    /// 解析后的数据品质
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
