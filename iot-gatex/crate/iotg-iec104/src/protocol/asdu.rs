use bytes::Bytes;
use chrono::{TimeZone, Utc};
use iotg_core::model::{DataPoint, Quality, Value};
use iotg_core::IotgError;
use wheel_rs::time_utils::get_current_timestamp;

/// 解析 ASDU，返回数据点列表
pub fn parse(
    driver: &str,
    ca_prefix: &str,
    ioa_prefix: &str,
    asdu: &Bytes,
) -> Result<Vec<DataPoint>, IotgError> {
    if asdu.len() < 9 {
        return Err(IotgError::Parse("asdu too short".to_string()));
    }
    let type_id = asdu[0];
    let sq_num = asdu[1];
    let sq = sq_num & 0x80 != 0;
    let n = (sq_num & 0x7F) as usize;
    // let cot   = asdu[2] & 0x3F; // 可用于过滤
    let ca_local = u16::from_le_bytes([asdu[4], asdu[5]]);
    let ca_used = ca_local;

    let device_id = format!("{}{}", ca_prefix, ca_used);
    let mut out = Vec::with_capacity(n);
    let mut off = 6usize;

    // 读第一个 IOA（3B），用于 SQ 顺序寻址
    let base_ioa = u32::from_le_bytes([asdu[off], asdu[off + 1], asdu[off + 2], 0]);

    for i in 0..n {
        let ioa = if sq {
            if i == 0 {
                off += 3;
                base_ioa
            } else {
                base_ioa + i as u32
            }
        } else {
            if off + 3 > asdu.len() {
                return Err(IotgError::Parse("apdu vsq number error".to_string()));
            }
            let v = u32::from_le_bytes([asdu[off], asdu[off + 1], asdu[off + 2], 0]);
            off += 3;
            v
        };

        let tag = format!("{}{}", ioa_prefix, ioa);

        let Some((value, quality, consumed, field_ts)) = parse_element(type_id, &asdu[off..])
        else {
            break;
        };
        off += consumed;

        let mut pt = DataPoint::builder()
            .driver(driver.to_string())
            .device_id(device_id.to_string())
            .tag(tag)
            .value(value)
            .quality(quality)
            .ts(get_current_timestamp()?)
            .field_ts(field_ts)
            .build();
        if let Some(ts) = field_ts {
            pt = pt.field_ts(Some(ts));
        }
        out.push(pt);
    }
    Ok(out)
}

fn parse_element(type_id: u8, data: &[u8]) -> Option<(Value, Quality, usize, Option<u64>)> {
    match type_id {
        // M_SP_NA_1 单点 (1B)
        1 => {
            let b = *data.first()?;
            Some((
                Value::Bool(b & 0x01 != 0),
                Quality::from_iec104_qds(b),
                1,
                None,
            ))
        }
        // M_SP_TB_1 带时标单点 (1+7=8B)
        30 => {
            if data.len() < 8 {
                return None;
            }
            let b = data[0];
            Some((
                Value::Bool(b & 0x01 != 0),
                Quality::from_iec104_qds(b),
                8,
                parse_cp56(&data[1..]),
            ))
        }
        // M_DP_NA_1 双点 (1B)
        3 => {
            let b = *data.first()?;
            Some((
                Value::Int((b & 0x03) as i64),
                Quality::from_iec104_qds(b),
                1,
                None,
            ))
        }
        // M_DP_TB_1 带时标双点 (8B)
        31 => {
            if data.len() < 8 {
                return None;
            }
            let b = data[0];
            Some((
                Value::Int((b & 0x03) as i64),
                Quality::from_iec104_qds(b),
                8,
                parse_cp56(&data[1..]),
            ))
        }
        // M_ME_NA_1 归一化 (3B)
        9 => {
            if data.len() < 3 {
                return None;
            }
            let raw = i16::from_le_bytes([data[0], data[1]]) as f64 / 32767.0;
            Some((
                Value::Float(raw),
                Quality::from_iec104_qds(data[2]),
                3,
                None,
            ))
        }
        // M_ME_NB_1 标度化 (3B)
        11 => {
            if data.len() < 3 {
                return None;
            }
            let raw = i16::from_le_bytes([data[0], data[1]]);
            Some((
                Value::Int(raw as i64),
                Quality::from_iec104_qds(data[2]),
                3,
                None,
            ))
        }
        // M_ME_NC_1 短浮点 (5B)
        13 => {
            if data.len() < 5 {
                return None;
            }
            let raw = f32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            Some((
                Value::Float(raw as f64),
                Quality::from_iec104_qds(data[4]),
                5,
                None,
            ))
        }
        // M_ME_TD_1 带时标归一化 (10B)
        34 => {
            if data.len() < 10 {
                return None;
            }
            let raw = i16::from_le_bytes([data[0], data[1]]) as f64 / 32767.0;
            Some((
                Value::Float(raw),
                Quality::from_iec104_qds(data[2]),
                10,
                parse_cp56(&data[3..]),
            ))
        }
        // M_ME_TF_1 带时标短浮点 (12B)
        36 => {
            if data.len() < 12 {
                return None;
            }
            let raw = f32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            Some((
                Value::Float(raw as f64),
                Quality::from_iec104_qds(data[4]),
                12,
                parse_cp56(&data[5..]),
            ))
        }
        // M_IT_NA_1 累积量 (5B)
        15 => {
            if data.len() < 5 {
                return None;
            }
            let raw = i32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            Some((
                Value::Int(raw as i64),
                Quality::from_iec104_qds(data[4] & 0xF0),
                5,
                None,
            ))
        }
        // M_BO_NA_1 32位比特串 (5B)
        7 => {
            if data.len() < 5 {
                return None;
            }
            let raw = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            Some((
                Value::Int(raw as i64),
                Quality::from_iec104_qds(data[4]),
                5,
                None,
            ))
        }
        // M_EI_NA_1 初始化结束 (1B)
        70 | 100 | 101 | 103 => Some((Value::Bool(true), Quality::GOOD, 1, None)),
        _ => {
            tracing::trace!("unsupported TypeID={}", type_id);
            None
        }
    }
}

fn parse_cp56(d: &[u8]) -> Option<u64> {
    if d.len() < 7 {
        return None;
    }
    let ms = u16::from_le_bytes([d[0], d[1]]) as u32;
    let min = (d[2] & 0x3F) as u32;
    let hour = (d[3] & 0x1F) as u32;
    let dom = (d[4] & 0x1F) as u32;
    let month = (d[5] & 0x0F) as u32;
    let year = 2000 + (d[6] & 0x7F) as i32;
    let sec = ms / 1000;
    Some(
        Utc.with_ymd_and_hms(year, month, dom, hour, min, sec)
            .single()?
            .timestamp_millis() as u64,
    )
}

/// 构建总召唤(General Interrogation，GI)指令 C_IC_NA_1
pub fn gi_cmd(qoi: u8) -> Bytes {
    Bytes::from(vec![100, 0x01, 0x06, 0x00, 0xFF, 0xFF, 0, 0, 0, qoi])
}

/// 构建电度召唤指令 C_CI_NA_1
pub fn kwh_cmd(qcc: u8) -> Bytes {
    Bytes::from(vec![101, 0x01, 0x06, 0x00, 0xFF, 0xFF, 0, 0, 0, qcc])
}
