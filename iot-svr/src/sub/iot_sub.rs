use crate::config::{AppConfig, IotConfig};
use config::Value as ConfigValue;
use influxdb::{Timestamp, WriteQuery};
use iot_api::dto::iot_mq_dto::{IotMqDto, Value};
use robotech::macros::log_call;
use robotech::mq::mqtt::{start_mqtt_subscriber, MqttError};
use robotech::tsdb::influxdb::build_influxdb_client;
use rumqttc::Publish;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::select;
use tokio::sync::mpsc;
use tokio::time::{Instant, sleep_until};
use tracing::{debug, error, info};
use wheel_rs::process::{get_current_pid, send_signal_by_instruction};

/// # 启动（或热更新）数据接入
///
/// 首次启动时建立 InfluxDB 客户端、启动缓存刷新任务与 MQTT 订阅者；
/// 当配置文件发生变更时，订阅与刷新任务无法在线热替换，这里选择优雅退出，
/// 由进程管理器以新配置重启。
///
/// ## 参数
/// * `app_config` - 最新的应用配置
/// * `changed` - 本次配置变更的差异；`None` 表示首次启动
///
/// ## 返回值
/// 启动成功返回 `Ok(())`；InfluxDB 客户端构建失败时返回 `Err(anyhow::Error)`
#[log_call]
pub async fn setup_iot_subscriber(
    app_config: Arc<AppConfig>,
    changed: &Option<HashMap<String, ConfigValue>>,
) -> anyhow::Result<()> {
    if changed.is_some() {
        info!("数据接入配置已更新，优雅退出以重新加载订阅");
        quit();
        return Ok(());
    }

    debug!("应用数据接入配置...");
    let AppConfig {
        iot: iot_config,
        mqtt: mqtt_config,
        influxdb: influxdb_config,
    } = app_config.as_ref().clone();
    let IotConfig {
        channel_capacity,
        flush_interval,
    } = iot_config;

    // 启动 InfluxDB 客户端
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
                        if let Err(e) = influxdb_client.query(write_queries).await {
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

    // 启动 MQTT 订阅者
    let _ = start_mqtt_subscriber(mqtt_config, move |publish| {
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
                        field_ms,
                        quality: _,
                    } = iot_mq_dto;
                    let measurement = match value {
                        Value::Bool(_) => "POINT-BOOL",
                        Value::U8(_) | Value::U32(_) | Value::I16(_) | Value::I32(_) => "POINT-I64",
                        Value::F32(_) => "POINT-F64",
                    };
                    let ns = if let Some(field_ms) = field_ms {
                        field_ms * 1_000_000
                    } else {
                        ns
                    };
                    debug!(
                        "解析出消息内容: driver={driver}, device={device}, metric={metric}, value={value}, ns={ns}"
                    );
                    let mut write_query =
                        WriteQuery::new(Timestamp::Nanoseconds(ns as u128), measurement)
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
                    write_query_sender
                        .send((metric.clone(), write_query))
                        .await
                        .map_err(|e| {
                            MqttError::Handle(format!("发送 write_query 进通道错误, {e}"))
                        })?;
                    Ok(())
                }
                Err(e) => {
                    return Err(MqttError::Handle(format!("消息JSON反序列化失败, {e}")));
                }
            }
        }
    })
    .await?;

    Ok(())
}

/// # 优雅退出当前进程
///
/// 向当前进程发送 `quit` 信号触发优雅退出。
fn quit() {
    let _ = send_signal_by_instruction("quit", get_current_pid());
}
