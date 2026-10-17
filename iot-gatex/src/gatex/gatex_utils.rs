use crate::config::{AppConfig, DriverConfig};
use anyhow::Context;
use config::Value;
use iotg_core::{Batch, Driver, CHANNEL_CAP};
use iotg_dlt645::Dlt645Driver;
use iotg_hj212::Hj212Driver;
use iotg_iec104::Iec104Driver;
use iotg_modbus::ModbusDriver;
use iotg_mqtt::run as mqtt_run;
use iotg_s7::S7Driver;
use robotech::macros::log_call;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, info};
use wheel_rs::process::{get_current_pid, send_signal_by_instruction};

/// # 应用（或热更新）网关配置
///
/// 首次启动时装配全部驱动实例与 MQTT sink；当配置文件发生变更时，
/// 由于驱动与 MQTT sink 均为常驻任务、无法在线热替换，这里选择优雅退出，
/// 由进程管理器以新配置重启。
///
/// ## 参数
/// * `app_config` - 最新的应用配置
/// * `changed` - 本次配置变更的差异；`None` 表示首次启动
///
/// ## 返回值
/// 装配成功返回 `Ok(())`；驱动构建失败时返回 `Err(anyhow::Error)`
#[log_call]
pub async fn setup_gatex(
    app_config: Arc<AppConfig>,
    changed: &Option<HashMap<String, Value>>,
) -> anyhow::Result<()> {
    if changed.is_some() {
        info!("网关配置已更新，优雅退出以重新加载驱动");
        quit();
        return Ok(());
    }

    debug!("应用网关配置...");
    let AppConfig { mqtt, drivers } = app_config.as_ref().clone();

    let (tx, rx) = mpsc::channel::<Batch>(CHANNEL_CAP);

    for driver_cfg in drivers {
        let driver = build_driver(driver_cfg).context("build driver")?;
        let tx = tx.clone();
        tokio::spawn(async move { driver.run(tx).await });
    }

    // 主持有的 tx 释放后，channel 在所有驱动退出时自然关闭
    drop(tx);

    // MQTT sink 阻塞当前 task，直到 channel 关闭
    tokio::spawn(async move { mqtt_run(mqtt, rx).await });

    Ok(())
}

/// # 根据配置构建驱动实例
///
/// ## 参数
/// * `cfg` - 驱动配置
///
/// ## 返回值
/// 对应的驱动实例
fn build_driver(cfg: DriverConfig) -> anyhow::Result<Box<dyn Driver>> {
    Ok(match cfg {
        DriverConfig::Iec104(c) => Box::new(Iec104Driver::new(c)),
        DriverConfig::Modbus(c) => Box::new(ModbusDriver::new(c)),
        DriverConfig::Dlt645(c) => Box::new(Dlt645Driver::new(c)),
        DriverConfig::S7(c) => Box::new(S7Driver::new(c)),
        DriverConfig::Hj212(c) => Box::new(Hj212Driver::new(c)),
    })
}

/// # 优雅退出当前进程
///
/// 向当前进程发送 `quit` 信号触发优雅退出。
fn quit() {
    let _ = send_signal_by_instruction("quit", get_current_pid());
}
