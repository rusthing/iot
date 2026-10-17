use iotg_dlt645::Dlt645Config;
use iotg_hj212::Hj212Config;
use iotg_iec104::Iec104Config;
use iotg_modbus::ModbusConfig;
use iotg_mqtt::MqttConfig;
use iotg_s7::S7Config;
use serde::Deserialize;

/// # 应用配置
///
/// 对应 `iot-gatex.toml`（或 `iot-gatex-{profile}.toml`）中的配置内容。
///
/// ## 字段
/// * `mqtt` - MQTT 发布端配置
/// * `drivers` - 驱动实例列表，每项通过 `type` 字段区分协议
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct AppConfig {
    /// MQTT 发布端配置
    #[serde(default)]
    pub mqtt: MqttConfig,
    /// 驱动实例列表
    #[serde(default)]
    pub drivers: Vec<DriverConfig>,
}

/// # 驱动实例配置
///
/// TOML 中用 `type = "iec104"` 等字段区分驱动类型。
#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DriverConfig {
    /// IEC 60870-5-104
    Iec104(Iec104Config),
    /// Modbus TCP
    Modbus(ModbusConfig),
    /// DL/T 645-2007
    Dlt645(Dlt645Config),
    /// Siemens S7
    S7(S7Config),
    /// HJ 212-2017
    Hj212(Hj212Config),
}
