# iot

物联网相关。多协议工业物联网数据采集（网关）与接入（服务端）。

## 项目结构

```
iot/
├── Cargo.toml            workspace（统一依赖与 [patch.crates-io]）
├── iot-api/              共享类型：cst 常量 + dto 线协议（网关 / 服务端共用）
├── iot-svr/              服务端：订阅 MQTT 采集数据并写入 InfluxDB
│   ├── src/config/       AppConfig + IotConfig
│   └── src/sub/          MQTT 订阅与 InfluxDB 批量写入
└── iot-gatex/            网关：多协议驱动采集并发布到 MQTT
    ├── src/config/       AppConfig + DriverConfig
    ├── src/gatex/        驱动装配与启动逻辑
    └── crate/iotg-*      各协议驱动实现
```

## 约定

- 启动入口统一由 `robotech::macros::bootstrap!(AppConfig)` 生成，各项目只实现 `setup()`。
- **网关与服务端都是「无 Web / 无 DB」的服务**，因此二者对 `robotech` 只启用
  `app` / `macros`（服务端另加 `mqtt` / `influxdb`），不引入 `db` / `web` /
  `config-center` / `registry-center` 相关能力。
- 依赖本地 `robotech-rs` / `wheel-rs`（见根 `Cargo.toml` 的 `[patch.crates-io]`）；
  用 cross 构建时需把该 `[patch.crates-io]` 注释掉。
- 采集数据点类型统一在 `iot-api` 定义，网关与服务端共用一份定义。
