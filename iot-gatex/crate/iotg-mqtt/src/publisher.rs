use std::collections::HashMap;
use std::sync::Arc;

use rumqttc::{AsyncClient, EventLoop, MqttOptions, QoS};
use serde_json::json;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};

use iotg_core::model::{Batch, DataPoint, Value};

use crate::config::MqttSinkConfig;

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
        "ts":       pt.ts,
        "field_ts": pt.field_ts.map(|t| t),
    })
    .to_string()
    .into_bytes()
}

/// 启动 MQTT eventloop（后台 task）+ 消费循环（当前 task）
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

    let (client, eventloop) = AsyncClient::new(opts, cfg.channel_capacity);

    // rumqttc eventloop 必须持续 poll 才能驱动内部发送队列
    tokio::spawn(drive_eventloop(eventloop));

    // 缓存，key = topic，用于同名覆盖
    let cache: Arc<RwLock<HashMap<String, DataPoint>>> = Arc::new(RwLock::new(HashMap::new()));
    let cache_clone = cache.clone();

    tokio::spawn(async move {
        let mut ticker = interval(flush_interval);
        loop {
            ticker.tick().await;
            let cached = cache_clone.read().await;
            if cached.is_empty() {
                continue;
            }
            for pt in cached.values() {
                let payload = serialize(pt);
                if let Err(e) = client.publish(&topic, qos, false, payload).await {
                    warn!("mqtt publish {topic}: {e}");
                }
            }
            let cached_len = cached.len();
            drop(cached);
            cache_clone.write().await.clear();
            debug!("mqtt sink: flushed {} points", cached_len);
        }
    });

    info!(host = %cfg.host, port = cfg.port, prefix = %topic_clone, "mqtt sink ready");

    while let Some(batch) = rx.recv().await {
        let mut cached = cache.write().await;
        for pt in batch {
            cached.insert(topic_clone.clone(), pt);
        }
    }

    info!("mqtt sink: channel closed, exiting");
}

async fn drive_eventloop(mut ev: EventLoop) {
    loop {
        match ev.poll().await {
            Ok(notification) => debug!("mqtt: {:?}", notification),
            Err(e) => {
                error!("mqtt eventloop: {e:#}");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}
