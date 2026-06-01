use anyhow::Context;
use clap::Parser;
use iot_gatex::app::{set_app_config, AppConfig, DriverConfig};
use iotg_core::{Batch, Driver, CHANNEL_CAP};
use iotg_dlt645::Dlt645Driver;
use iotg_hj212::Hj212Driver;
use iotg_iec104::Iec104Driver;
use iotg_modbus::ModbusDriver;
use iotg_mqtt::run as mqtt_run;
use iotg_s7::S7Driver;
use log::debug;
use robotech::app::{build_app_cfg, wait_app_exit};
use robotech::cfg::watch_cfg_file;
use robotech::env::init_env;
use robotech::log::init_log;
use robotech::macros::{log_call, watch_cfg_file};
use robotech::signal::SignalManager;
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Parser, Debug, Clone)]
// 命令行参数使用定义
// version: 命令行添加 -V/--version参数可以查看版本信息
// about: --help命令第一行显示文档注释的内容
// long_about = None: 只显示文档注释的第一行(包括about的和arg的)
// help_template: 帮助模板
# [command(
                  author = env!("CARGO_PKG_AUTHORS"),
                  version,
                  about,
                  help_template = "{name} v{version} - {about}\n\nAUTHOR: {author}\n\nUSAGE: {usage}\n\nOPTIONS:\n{options}"
                  )]
struct Args {
    /// 配置文件的路径
    #[arg(short, long)]
    config_file: Option<String>,
    /// 监听信号，支持指令如下:
    /// * `start` - 默认值，先发送`SIGCONT`信号(kill -0)，检查程序是否已运行(如果程序已运行，会报错)，然后启动程序
    /// * `restart` - 先发送`SIGTERM`信号(kill -15)，如果旧程序已运行，收到信号后会停止运行，然后启动新程序
    /// * `stop`/`s` - 发送`SIGTERM`信号(kill -15)，用于终止程序，优雅退出
    /// * `kill`/`k` - 发送`SIGKILL`信号(kill -9)，用于强制终止程序
    #[arg(
        short,
        long,
        default_value = "start",
        long_help = r#"监听信号，支持指令如下:
    start - 默认值，先发送 SIGCONT 信号(kill -0)，检查程序是否已运行(如果程序已运行，会报错)，然后启动程序
    restart - 先发送 SIGTERM 信号(kill -15)，如果旧程序已运行，收到信号后会停止运行，然后启动新程序
    stop/s - 发送 SIGTERM 信号(kill -15)，用于终止程序，优雅退出
    kill/k - 发送 SIGKILL 信号(kill -9)，用于强制终止程序"#
    )]
    signal: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 解析命令行参数
    let Args {
        signal,
        config_file,
    } = Args::parse();

    // 初始化环境变量;
    init_env()?;
    // 初始化日志系统
    init_log()?;

    // 初始化信号(_signal_manager变量将在程序优雅退出时释放，释放时删除pid文件)
    let (mut signal_manager, old_pid) = SignalManager::new(signal)?;
    let (app_config, files) = build_app_cfg::<AppConfig>(config_file.clone())?;
    let files = Arc::new(files);

    // 监听配置文件变化
    watch_cfg_file!("app", files.clone(), {
        let (app_config, _) =
            build_app_cfg::<AppConfig>(config_file.clone()).expect("无法加载配置文件");
        apply_app_config(app_config, None)
            .await
            .expect("配置无法应用");
        debug!("重新加载配置成功");
    });

    // 应用配置
    apply_app_config(app_config, old_pid).await?;

    // 监听系统信号与等待退出
    let signal_receiver = signal_manager.watch_signal()?;
    Ok(wait_app_exit(signal_receiver, || async move {
        // stop_web_service().await.expect("无法停止旧的Web服务");
        Ok(())
    })
    .await?)
}

#[log_call]
async fn apply_app_config(app_config: AppConfig, old_pid: Option<u32>) -> anyhow::Result<()> {
    debug!("应用App配置...");
    let AppConfig { mqtt, drivers } = app_config.clone();
    set_app_config(app_config)?;

    let (tx, rx) = mpsc::channel::<Batch>(CHANNEL_CAP);

    for driver_cfg in drivers {
        let driver = build_driver(driver_cfg).context("build driver")?;
        let tx = tx.clone();
        tokio::spawn(async move { driver.run(tx).await });
    }

    // 主持有的 tx 释放后，channel 在所有驱动退出时自然关闭
    drop(tx);

    // MQTT sink 阻塞当前 task，直到 channel 关闭
    mqtt_run(mqtt, rx).await;

    Ok(())
}

fn build_driver(cfg: DriverConfig) -> anyhow::Result<Box<dyn Driver>> {
    Ok(match cfg {
        DriverConfig::Iec104(c) => Box::new(Iec104Driver::new(c)),
        DriverConfig::Modbus(c) => Box::new(ModbusDriver::new(c)),
        DriverConfig::Dlt645(c) => Box::new(Dlt645Driver::new(c)),
        DriverConfig::S7(c) => Box::new(S7Driver::new(c)),
        DriverConfig::Hj212(c) => Box::new(Hj212Driver::new(c)),
    })
}
