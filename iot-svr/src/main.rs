use clap::Parser;
use influxdb::{Timestamp, WriteQuery};
use iot_svr::app::iot_config::IotConfig;
use iot_svr::app::AppConfig;
use iot_svr::dto::iot_mq_dto::{IotMqDto, Value};
use robotech;
use robotech::app::{build_app_cfg, wait_app_exit};
use robotech::cfg::watch_cfg_file;
use robotech::env::init_env;
use robotech::log::init_log;
use robotech::macros::{log_call, watch_cfg_file};
use robotech::mq::mqtt::{start_mqtt_subscriber, MqttError};
use robotech::signal::SignalManager;
use robotech::tsdb::influxdb::build_influxdb_client;
use rumqttc::{AsyncClient, Publish};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::select;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{sleep_until, Instant};
use tracing::{debug, error, info};
use wheel_rs::process::{get_current_pid, send_signal_by_instruction};

// 命令行参数使用定义
// version: 命令行添加 -V/--version参数可以查看版本信息
// about: --help命令第一行显示文档注释的内容
// long_about = None: 只显示文档注释的第一行(包括about的和arg的)
// help_template: 帮助模板
#[derive(Parser, Debug, Clone)]
#[command(
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

    // 应用配置
    let (mqtt_client, mqtt_event_loop_handle) = apply_app_config(app_config, old_pid).await?;

    let mqtt_client_clone = mqtt_client.clone();
    let mqtt_event_loop_handle_clone = mqtt_event_loop_handle.clone();
    // 监听配置文件变化
    watch_cfg_file!("app", files.clone(), {
        let (app_config, _) =
            build_app_cfg::<AppConfig>(config_file.clone()).expect("无法加载配置文件");
        info!("配置文件已更新，优雅退出");
        quit();
    });

    // 监听系统信号与等待退出
    let signal_receiver = signal_manager.watch_signal()?;
    let mqtt_client_clone = mqtt_client.clone();
    let mqtt_event_loop_handle_clone = mqtt_event_loop_handle.clone();
    Ok(wait_app_exit(signal_receiver, || async {
        mqtt_client_clone.disconnect().await.ok(); // 主动断开连接
        mqtt_event_loop_handle_clone.abort(); // 取消事件循环任务
        Ok(())
    })
    .await?)
}

/// # 应用配置
///
/// ## Arguments
/// * `port` - 一个可选的u16值，指定Web服务器监听的端口。如果未指定，则使用配置文件中的设置或默认值。
/// * `old_pid` - 一个可选的i32值，代表旧进程ID，用于在重启时清理资源等操作。
///
/// ## Functionality
/// 1. 加载并构建应用配置信息。
/// 2. 将配置信息保存到全局上下文中以供其他部分访问。
/// 3. 根据配置中的数据库设置执行数据库迁移以确保数据库结构是最新的。
/// 4. 初始化ID生成器，可能用于生成全局唯一ID。
/// 5. 建立与数据库的连接。
/// 6. 使用提供的或默认的端口号启动Web服务器，并处理任何给定的旧进程ID。
///
/// ## Errors
/// 如果在升级数据库版本时遇到问题，将打印错误信息并终止程序执行。
///
/// ## Examples
/// ```ignore
/// // 使用默认配置和端口初始化配置
/// init_config(None, None, None).await;
///
/// // 指定配置文件路径、自定义端口和旧进程ID来初始化配置
/// init_config(Some(String::from("path/to/app.toml")), Some(8080), Some(1234)).await;
/// ```
///
#[log_call]
async fn apply_app_config(
    app_config: AppConfig,
    old_pid: Option<u32>,
) -> anyhow::Result<(Arc<AsyncClient>, Arc<JoinHandle<()>>)> {
    debug!("应用App配置...");
    let AppConfig {
        iot: iot_config,
        mqtt: mqtt_config,
        influxdb: influxdb_config,
    } = app_config;
    let IotConfig {
        channel_capacity,
        flush_interval,
    } = iot_config;

    // 启动InfluxDB客户端
    let influxdb_client = build_influxdb_client(influxdb_config)?;

    // 缓存，key = metric
    let mut write_query_cache: HashMap<String, WriteQuery> = HashMap::new();
    let mut next_flush = Instant::now() + flush_interval;
    let (write_query_sender, mut write_query_receiver) =
        mpsc::channel::<(String, WriteQuery)>(channel_capacity);

    tokio::spawn(async move {
        loop {
            select! {
                // 接收 write_query channel
                Some((metric, write_query)) = write_query_receiver.recv() => {
                    write_query_cache.insert(metric, write_query);
                }
                // 定时刷新缓存
                _ = sleep_until(next_flush) => {
                    if !write_query_cache.is_empty() {
                        debug!("write query cache {} points", write_query_cache.len());
                        let mut write_queries = vec![];
                        for write_query in write_query_cache.values() {
                            write_queries.push(write_query.clone());
                        }
                        debug!("写入influxdb数据库: {write_queries:?}");
                        if let Err(e) = influxdb_client
                            .query(write_queries)
                            .await
                        {
                            error!("插入InfluxDB数据库失败, {e}");
                        } else {
                            debug!("write query flushed {} points", write_query_cache.len());
                            write_query_cache.clear();
                        }
                    }
                    // 更新下次刷新时间
                    next_flush = Instant::now() + flush_interval;
                }
            }
        }
    });

    // 启动MQTT订阅者
    Ok(start_mqtt_subscriber(mqtt_config, move |publish| {
        let write_query_sender = write_query_sender.clone();
        async move {
            let Publish { payload, .. } = publish;
            match serde_json::from_slice::<IotMqDto>(&payload) {
                Ok(iot_mq_dto) => {
                    let IotMqDto {
                        driver,
                        device,
                        metric,
                        value,
                        ns,
                        field_ts,
                        quality: _,
                    } = iot_mq_dto;
                    let measurement = match value {
                        Value::Bool(_) => "POINT-BOOL",
                        Value::U8(_) | Value::U32(_) | Value::I16(_) | Value::I32(_) => "POINT-I64",
                        Value::F32(_) => "POINT-F64",
                    };
                    let ns = if let Some(field_ts) = field_ts { field_ts * 1_000_000 } else { ns };
                    debug!("解析出消息内容: driver={driver}, device={device}, metric={metric}, value={value}, ns={ns}");
                    let mut write_query = WriteQuery::new(Timestamp::Nanoseconds(ns as u128), measurement)
                        .add_tag("driver", driver)
                        .add_tag("device", device)
                        .add_tag("metric", metric.clone());
                    write_query = match value {
                        Value::Bool(b) => write_query.add_field("value", b),
                        Value::U8(u) => write_query.add_field("value", u as i64),
                        Value::U32(u) => write_query.add_field("value", u as i64),
                        Value::I16(u) => write_query.add_field("value", u as i64),
                        Value::I32(u) => write_query.add_field("value", u as i64),
                        Value::F32(u) => write_query.add_field("value", u as f64),
                    };
                    write_query_sender.send((metric.clone(), write_query)).await.map_err(
                        |e|MqttError::Handle(format!("发送 write_query 进通道错误, {e}")))?;
                    Ok(())
                }
                Err(e) => {
                    return Err(MqttError::Handle(format!("消息JSON反序列化失败, {e}")));
                }
            }
        }
    }).await?)
}

fn quit() {
    let _ = send_signal_by_instruction("quit", get_current_pid());
}
