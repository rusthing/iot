use bytes::Bytes;
use chrono::{TimeZone, Utc};
use hex::encode_upper;
use iotg_core::model::{DataPoint, Quality, Value};
use iotg_core::IotgError;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use tracing::{debug, info, warn};
use wheel_rs::time_utils::now_ns;

// 传送原因
#[derive(Debug, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum CotType {
    /// 255: 不支持的类型
    NotSupport = 255,
    /// 0: 未定义
    Undefined = 0,
    /// 1: 周期、循环
    Periodic = 1,
    /// 2: 背景扫描
    BackgroundScan = 2,
    /// 3: 突发(自发)
    Spontaneous = 3,
    /// 4: 初始化
    Init = 4,
    /// 5: 请求、查询
    Request = 5,
    /// 6: 激活（下发遥控/设点命令）
    Activation = 6,
    /// 7: 激活确认（主站下发命令后子站回复确认）
    ActivationConfirm = 7,
    /// 8: 停止激活
    Deactivation = 8,
    /// 9: 停止激活确认
    DeactivationConfirm = 9,
    /// 10 终止激活
    TerminationOfActivation = 10,
    /// 20 响应站召唤
    ResponseGi = 20,
    /// 37 响应计数量站召唤
    ResponseKwh = 37,
}

impl CotType {
    /// 转中文说明
    pub fn to_cn(&self) -> &'static str {
        match self {
            CotType::Undefined => "未定义",
            CotType::Periodic => "周期循环",
            CotType::BackgroundScan => "背景扫描",
            CotType::Spontaneous => "突发上送",
            CotType::Init => "初始化",
            CotType::Request => "查询请求",
            CotType::Activation => "激活",
            CotType::ActivationConfirm => "激活确认",
            CotType::Deactivation => "停止激活",
            CotType::DeactivationConfirm => "停止激活确认",
            CotType::TerminationOfActivation => "终止激活",
            CotType::ResponseGi => "响应站召唤",
            CotType::ResponseKwh => "响应计数量站召唤",
            _ => "未支持的传送原因类型",
        }
    }
}

/// 解析 ASDU，返回数据点列表
pub fn parse(driver: &str, device: &str, asdu: &Bytes) -> Result<Vec<DataPoint>, IotgError> {
    if asdu.len() < 9 {
        return Err(IotgError::Parse("asdu too short".to_string()));
    }
    let type_id = asdu[0];
    let sq_num = asdu[1];
    let sq = sq_num & 0x80 != 0;
    let sq_num = (sq_num & 0x7F) as usize;
    let cot = asdu[2] & 0x3F; // 可用于过滤
    let ca = u16::from_le_bytes([asdu[4], asdu[5]]);

    let mut out = Vec::with_capacity(sq_num);
    let mut off = 6usize;

    // 读第一个 IOA（3B），用于 SQ 顺序寻址
    let mut ioa = u32::from_le_bytes([asdu[off], asdu[off + 1], asdu[off + 2], 0]);

    debug!(
        "parse asdu: ti={:} sq={} sq_num={} cot={} ca={} ioa={}",
        type_id, sq, sq_num, cot, ca, ioa
    );

    for i in 0..sq_num {
        ioa = if sq {
            if i == 0 {
                off += 3;
                ioa
            } else {
                ioa + i as u32
            }
        } else {
            if off + 3 > asdu.len() {
                return Err(IotgError::Parse("apdu vsq number error".to_string()));
            }
            let v = u32::from_le_bytes([asdu[off], asdu[off + 1], asdu[off + 2], 0]);
            off += 3;
            v
        };

        let metric = format!("{}-{}", ca, ioa);

        let cot_text = CotType::try_from_primitive(cot)
            .unwrap_or(CotType::NotSupport)
            .to_cn();
        let Some((value, quality, consumed, field_ts)) =
            parse_element(type_id, cot_text, &asdu[off..])
        else {
            break;
        };
        off += consumed;

        debug!(
            "parse element: metric={metric} value={value:?} quality={quality:?} consumed={consumed} field_ts={field_ts:?}"
        );

        let mut pt = DataPoint::builder()
            .driver(driver.to_string())
            .device(device.to_string())
            .metric(metric)
            .value(value)
            .quality(quality)
            .ns(now_ns()?)
            .field_ts(field_ts)
            .build();
        if let Some(ts) = field_ts {
            pt = pt.field_ts(Some(ts));
        }
        out.push(pt);
    }
    Ok(out)
}

fn parse_element(
    type_id: u8,
    cot_text: &str,
    data: &[u8],
) -> Option<(Value, Quality, usize, Option<u64>)> {
    debug!(
        "parse_element: type_id={} data={}",
        type_id,
        encode_upper(data)
    );
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
        // M_IT_TB_1 带时标累积量 (12B)
        38 => {
            if data.len() < 12 {
                return None;
            }
            let raw = i32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            Some((
                Value::Int(raw as i64),
                Quality::from_iec104_qds(data[4] & 0xF0),
                12,
                parse_cp56(&data[5..]),
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
        70 => {
            info!("received initialization end, cot={cot_text}");
            Some((Value::Bool(true), Quality::GOOD, 1, None))
        }
        100 => {
            info!("received general interrogation, cot={cot_text}");
            Some((Value::Bool(true), Quality::GOOD, 1, None))
        }
        101 => {
            info!("received kwh interrogation, cot={cot_text}");
            Some((Value::Bool(true), Quality::GOOD, 1, None))
        }
        103 => {
            info!("received clock sync, cot={cot_text}");
            Some((Value::Bool(true), Quality::GOOD, 1, None))
        }
        _ => {
            warn!("unsupported ti={type_id}: data={data:?}");
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
