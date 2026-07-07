use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde_json::json;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};

use crate::config::MqttSinkConfig;
use iotg_core::model::{Batch, DataPoint, Value};

fn to_qos(q: u8) -> QoS {
    match q {
        0 => QoS::AtMostOnce,
        2 => QoS::ExactlyOnce,
        _ => QoS::AtLeastOnce,
    }
}

fn serialize(pt: &DataPoint) -> Vec<u8> {
    let v = match &pt.value {
        Value::Bool(b) => json!(b),
        Value::Int(i) => json!(i),
        Value::Float(f) => json!(f),
        Value::Text(s) => json!(s),
        Value::Bytes(b) => json!(hex::encode(b)),
    };
    json!({
        "driver": pt.driver,
        "device": pt.device,
        "metric": pt.metric,
        "value" : v,
        "quality": {
            "good":        pt.quality.good,
            "invalid":     pt.quality.invalid,
            "not_topical": pt.quality.not_topical,
            "substituted": pt.quality.substituted,
            "overflow":    pt.quality.overflow,
        },
        "ns":       pt.ns,
        "field_ts": pt.field_ts.map(|t| t),
    })
    .to_string()
    .into_bytes()
}

use tokio::select;

/// 启动 MQTT eventloop + 消费循环
pub async fn run(cfg: MqttSinkConfig, mut rx: mpsc::Receiver<Batch>) {
    let qos = to_qos(cfg.qos);
    let topic = cfg.topic.clone();
    let topic_clone = topic.clone();
    let flush_interval = cfg.flush_interval;

    let mut opts = MqttOptions::new(&cfg.client_id, &cfg.host, cfg.port);
    opts.set_keep_alive(Duration::from_secs(cfg.keepalive_secs));
    opts.set_clean_session(true);
    if let (Some(u), Some(p)) = (&cfg.username, &cfg.password) {
        opts.set_credentials(u, p);
    }

    let (client, mut eventloop) = AsyncClient::new(opts, cfg.channel_capacity);

    // 缓存，key = topic
    let mut cache: HashMap<String, DataPoint> = HashMap::new();
    let mut ticker = interval(flush_interval);

    info!(host = %cfg.host, port = cfg.port, prefix = %topic_clone, "mqtt sink ready");

    loop {
        select! {
            // 1. 处理 MQTT 事件循环
            notification = eventloop.poll() => {
                match notification {
                    Ok(n) => debug!("mqtt: {:?}", n),
                    Err(e) => {
                        error!("mqtt eventloop: {e:#}");
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                }
            }

            // 2. 接收新数据
            Some(batch) = rx.recv() => {
                for pt in batch {
                    cache.insert(topic_clone.clone(), pt);
                }
            }

            // 3. 定时刷新缓存
            _ = ticker.tick() => {
                if cache.is_empty() {
                    continue;
                }

                for pt in cache.values() {
                    let payload = serialize(pt);
                    if let Err(e) = client.publish(&topic, qos, false, payload).await {
                        warn!("mqtt publish {topic}: {e}");
                    }
                    info!("mqtt published {topic}: {pt}");
                }

                debug!("mqtt sink: flushed {} points", cache.len());
                cache.clear();
            }

            // 4. 所有 channel 关闭时退出
            else => {
                info!("mqtt sink: channel closed, exiting");
                break;
            }
        }
    }
}
