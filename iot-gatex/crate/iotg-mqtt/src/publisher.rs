use crate::config::MqttConfig;
use iotg_core::iotg_dto::{Batch, IotMqDto};
use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde_json::json;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::time::{sleep_until, Duration, Instant};
use tracing::{debug, error, info, warn};

fn to_qos(q: u8) -> QoS {
    match q {
        0 => QoS::AtMostOnce,
        2 => QoS::ExactlyOnce,
        _ => QoS::AtLeastOnce,
    }
}

// fn serialize(pt: &DataPoint) -> Vec<u8> {
//     let value = match &pt.value {
//         Value::Bool(b) => json!(b),
//         Value::U8(u) => json!(u),
//         Value::U32(u) => json!(u),
//         Value::I16(i) => json!(i),
//         Value::I32(i) => json!(i),
//         Value::F32(f) => json!(f),
//     };
//     json!({
//         "driver": pt.driver,
//         "device": pt.device,
//         "metric": pt.metric,
//         "value" : pt.value,
//         "quality": {
//             "good":        pt.quality.good,
//             "invalid":     pt.quality.invalid,
//             "notTopical": pt.quality.not_topical,
//             "substituted": pt.quality.substituted,
//             "overflow":    pt.quality.overflow,
//         },
//         "ns":       pt.ns,
//         "fieldTs": pt.field_ts.map(|t| t),
//     })
//     json!(pt).to_string().into_bytes()
// }

use tokio::select;

/// 启动 MQTT eventloop + 消费循环
pub async fn run(cfg: MqttConfig, mut rx: mpsc::Receiver<Batch>) {
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
    let mut cache: HashMap<String, IotMqDto> = HashMap::new();
    let mut next_flush = Instant::now() + flush_interval;

    info!(host = %cfg.host, port = cfg.port, prefix = %topic_clone, "mqtt ready");

    loop {
        select! {
            // 1. 处理 MQTT 事件循环
            notification = eventloop.poll() => {
                match notification {
                    Ok(n) => debug!("mqtt poll: {:?}", n),
                    Err(e) => {
                        error!("mqtt eventloop: {e:#}");
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                }
            }

            // 2. 接收新数据
            Some(batch) = rx.recv() => {
                for pt in batch {
                    cache.insert(pt.metric.clone(), pt);
                }
            }

            // 3. 定时刷新缓存
            _ = sleep_until(next_flush) => {
                if !cache.is_empty() {
                    debug!("mqtt cache {} points", cache.len());
                    for pt in cache.values() {
                        let json = json!(pt).to_string();
                        debug!("mqtt will publish {json}");
                        let payload = json.as_bytes();
                        if let Err(e) = client.publish(&topic, qos, false, payload).await {
                            warn!("mqtt publish {topic}: {e}");
                        }
                        debug!("mqtt published {topic}: {pt}");
                    }

                    debug!("mqtt flushed {} points", cache.len());
                    cache.clear();
                }
                // 更新下次刷新时间
                next_flush = Instant::now() + flush_interval;
            }
        }
    }
}
