use iotg_dlt645::Dlt645Config;
use iotg_hj212::Hj212Config;
use iotg_iec104::Iec104Config;
use iotg_modbus::ModbusConfig;
use iotg_mqtt::MqttSinkConfig;
use iotg_s7::S7Config;
use robotech::app::AppError;
use serde::Deserialize;
use std::sync::RwLock;

static APP_CONFIG: RwLock<Option<AppConfig>> = RwLock::new(None);

/// 获取App配置的只读访问
pub fn get_app_config() -> Result<AppConfig, AppError> {
    let read_lock = APP_CONFIG.read().map_err(|_| AppError::GetAppConfig())?;
    read_lock.clone().ok_or(AppError::GetAppConfig())
}

/// 设置App配置
pub fn set_app_config(value: AppConfig) -> Result<(), AppError> {
    let mut write_lock = APP_CONFIG.write().map_err(|_| AppError::SetAppConfig())?;
    *write_lock = Some(value);
    Ok(())
}

/// 配置文件结构
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct AppConfig {
    #[serde(default)]
    pub mqtt: MqttSinkConfig,
    #[serde(default)]
    pub drivers: Vec<DriverConfig>,
}

/// TOML 中用 `type = "iec104"` 等字段区分驱动类型
#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
enum DriverConfig {
    Iec104(Iec104Config),
    Modbus(ModbusConfig),
    Dlt645(Dlt645Config),
    S7(S7Config),
    Hj212(Hj212Config),
}
