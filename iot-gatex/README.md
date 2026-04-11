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
├── src/main.rs              配置加载、驱动装配、启动
├── config.toml              运行时配置（见下文）
└── crates/
    ├── iotg-core/           Driver trait + DataPoint + Value + Quality
    ├── iotg-iec104/         IEC 60870-5-104 完整实现
    ├── iotg-modbus/         Modbus TCP 骨架
    ├── iotg-dlt645/         DLT 645-2007 骨架
    ├── iotg-s7/             Siemens S7 骨架
    ├── iotg-hj212/          HJ 212-2017 骨架
    └── iotg-mqtt/           MQTT Sink（rumqttc）
```

## 数据模型

所有协议统一输出 `DataPoint`：

```rust
pub struct DataPoint {
    pub driver:    String,          // 驱动实例名（来自配置 name）
    pub device_id: String,          // 设备标识（如 "ca1"、"unit1"）
    pub tag:       String,          // 数据标签（如 "ioa1001"、"hr40001"）
    pub value:     Value,           // Bool / Int / Float / Text / Bytes
    pub quality:   Quality,         // good / invalid / not_topical / ...
    pub ts:        DateTime<Utc>,   // 本地接收时间
    pub field_ts:  Option<...>,     // 设备携带时标（如有）
}
```

MQTT topic 格式：`{prefix}/{driver}/{device_id}/{tag}`

MQTT payload（JSON）：

```json
{
  "driver":    "substation-a",
  "device_id": "ca1",
  "tag":       "ioa1001",
  "value":     220.4,
  "quality":   { "good": true, "invalid": false, "not_topical": false,
                 "substituted": false, "overflow": false },
  "ts":        "2026-04-10T12:00:00Z",
  "field_ts":  null
}
```

## 快速开始

```bash
# 1. 编辑配置
cp config.toml.example config.toml
vim config.toml

# 2. 编译运行
cargo build --release
./target/release/iot-gatex

# 3. 调整日志级别
RUST_LOG=debug ./target/release/iot-gatex

# 4. 订阅所有数据
mosquitto_sub -h 127.0.0.1 -t "gatex/#" -v
```

## config.toml 说明

```toml
[mqtt]
host          = "127.0.0.1"
port          = 1883
client_id     = "iot-gatex"
topic_prefix  = "gatex"
qos           = 1             # 0 / 1 / 2

# IEC-104 从站
[[drivers]]
type             = "iec104"
name             = "substation-a"   # 唯一实例名，出现在 topic 和日志中
host             = "192.168.1.10"
port             = 2404
ca               = 1
auto_interrogate = true
qoi              = 20               # 20 = 全站总召唤

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

1. 在 `crates/` 下新建 `iotg-<proto>/`，结构与 `iotg-modbus` 相同
2. 实现 `Driver` trait（`iotg-core::Driver`），核心只有一个方法：
   ```rust
   async fn run(self: Box<Self>, tx: mpsc::Sender<Batch>);
   ```
3. 在 `Cargo.toml` workspace `members` 中添加新 crate
4. 在 `src/main.rs` 的 `DriverConfig` enum 和 `build_driver` 中加一个分支
5. 在 `config.toml` 中用新的 `type = "<proto>"` 声明实例

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
