# iot-gatex

多协议工业物联网网关，采集数据实时发布至 MQTT。

## 支持协议

| 协议    | crate          | 状态     | 连接方式         |
|---------|----------------|----------|-----------------|
| IEC-104 | `iotg-iec104`  | 完整实现 | TCP Client       |
| Modbus  | `iotg-modbus`  | 骨架     | TCP Client       |
| DLT645  | `iotg-dlt645`  | 骨架     | TCP/串口         |
| S7      | `iotg-s7`      | 骨架     | ISO-on-TCP (102) |
| HJ212   | `iotg-hj212`   | 骨架     | TCP Server       |

## 项目结构

```
iot-gatex/
├── src/main.rs              bootstrap! + setup()（启动入口）
├── src/config/              AppConfig + DriverConfig（应用配置）
├── src/gatex/               驱动装配与启动逻辑（setup_gatex）
├── iot-gatex.toml           运行时配置（见下文）
└── crate/
    ├── iotg-core/           Driver trait + DataPoint + Value + Quality
    ├── iotg-iec104/         IEC 60870-5-104 完整实现
    ├── iotg-modbus/         Modbus TCP 骨架
    ├── iotg-dlt645/         DLT 645-2007 骨架
    ├── iotg-s7/             Siemens S7 骨架
    ├── iotg-hj212/          HJ 212-2017 骨架
    └── iotg-mqtt/           MQTT Sink（rumqttc）
```

> 采集数据点类型（`IotMqDto` / `Value` / `Quality`）统一定义在 `iot-api` crate 中，
> `iotg-core` 直接转发，网关与服务端共用一份定义。

## 数据模型

所有协议统一输出采集数据点 `IotMqDto`（定义在 `iot-api`，网关与服务端共用）：

```rust
pub struct IotMqDto {
    pub driver:   String,          // 驱动实例名（来自配置 name）
    pub device:   String,          // 设备标识（如 "ca1"、"unit1"、"slave3"）
    pub metric:   String,          // 指标（如 "ioa1001"、"hr40001"）
    pub value:    Value,           // Bool / U8 / U32 / I16 / I32 / F32
    pub quality:  Quality,         // good / invalid / notTopical / substituted / overflow
    pub ns:       u64,             // 采集器接收时间（纳秒）
    pub field_ms: Option<u64>,     // 设备携带时标（毫秒，如有）
}
```

序列化为 camelCase JSON 后发布到配置的 `[mqtt].topic`：

```json
{
  "driver": "substation-a",
  "device": "ca1",
  "metric": "ioa1001",
  "value": { "kind": "F32", "value": 220.4 },
  "quality": { "good": true, "invalid": false, "notTopical": false,
               "substituted": false, "overflow": false },
  "ns": 1760000000000000000,
  "fieldMs": 1760000000000
}
```

## 快速开始

```bash
# 1. 编辑配置
vim iot-gatex.toml

# 2. 编译运行
cargo build --release
./target/release/iot-gatex

# 3. 调整日志级别
RUST_LOG=debug ./target/release/iot-gatex

# 4. 订阅所有数据
mosquitto_sub -h 127.0.0.1 -t "iot-gatex" -v
```

## iot-gatex.toml 说明

```toml
[mqtt]
client-id = "iot-gatex"
host      = "127.0.0.1"
port      = 1883
topic     = "iot-gatex"
qos       = 1             # 0 / 1 / 2

# IEC-104 从站
[[drivers]]
type    = "iec104"
name    = "substation-a"    # 唯一实例名，出现在日志中
host    = "192.168.1.10"
port    = 2404
get-gi  = true              # 启动后自动总召唤
qoi     = 20                # 20 = 全站总召唤

# Modbus TCP
[[drivers]]
type         = "modbus"
name         = "plc-line1"
host         = "192.168.1.20"
port         = 502
unit_id      = 1
interval_ms  = 1000

  [[drivers.polls]]
  kind  = "holding"   # coil / discrete / holding / input
  start = 0
  count = 20

# DLT645 电表（TCP 透传）
[[drivers]]
type        = "dlt645"
name        = "meter-a"
host        = "192.168.1.30"
meter_addr  = "000000123456"
data_ids    = ["00010000", "02010100"]   # 正向有功、A 相电压

# Siemens S7
[[drivers]]
type        = "s7"
name        = "s7-filling"
host        = "192.168.1.40"
slot        = 1

  [[drivers.reads]]
  tag         = "speed"
  area        = "DB"
  db_number   = 1
  byte_offset = 0
  data_type   = "REAL"

# HJ212 环保数采仪（被动监听）
[[drivers]]
type         = "hj212"
name         = "env-site1"
listen_host  = "0.0.0.0"
listen_port  = 7070
```

## 新增协议

1. 在 `crate/` 下新建 `iotg-<proto>/`，结构与 `iotg-modbus` 相同
2. 实现 `Driver` trait（`iotg-core::Driver`），核心只有一个方法：
   ```rust
   async fn run(self: Box<Self>, tx: mpsc::Sender<Batch>);
   ```
3. 在 `Cargo.toml` workspace `members` 中添加新 crate
4. 在 `src/config/app_config.rs` 的 `DriverConfig` enum 和 `src/gatex/gatex_utils.rs`
   的 `build_driver` 中加一个分支
5. 在 `iot-gatex.toml` 中用新的 `type = "<proto>"` 声明实例

**不需要修改任何其他 crate**，MQTT sink、channel、日志全部自动继承。

## 架构说明

```
设备 ──TCP/串口──► Driver::run()
                      │  mpsc::Sender<Batch>
                      ▼
              mpsc channel (有界, 8192)
                      │  mpsc::Receiver<Batch>
                      ▼
              iotg-mqtt::run()
                      │  rumqttc::AsyncClient::publish
                      ▼
              MQTT Broker
```

- 每个驱动实例独占一个 `tokio::task`，互相隔离，单驱动崩溃不影响其他驱动
- 所有驱动共享同一条 `mpsc` channel，背压天然传导到最慢的驱动
- MQTT eventloop 单独一个 task，与 publish 调用解耦，避免发送阻塞
- 新增/删除驱动只修改配置文件，无需重编译
