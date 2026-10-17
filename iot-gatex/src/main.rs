use config::Value;
use iot_gatex::config::AppConfig;
use iot_gatex::gatex::setup_gatex;
use robotech::macros::{bootstrap, log_call};
use std::collections::HashMap;
use std::sync::Arc;

bootstrap!(AppConfig);

/// # 初始化或更新应用配置
///
/// 由 `bootstrap!` 生成的 `main()` 在首次启动与配置热更新时调用。
///
/// ## 参数
/// * `app_config` - 最新的应用配置
/// * `changed` - 本次配置变更的差异；`None` 表示首次启动
/// * `_port` - Web 端口（网关无 Web 服务，未使用）
/// * `_old_pid` - 旧进程 PID（未使用）
#[log_call]
async fn setup(
    app_config: &Arc<AppConfig>,
    changed: &Option<HashMap<String, Value>>,
    _port: Option<u16>,
    _old_pid: Option<u32>,
) -> Result<(), anyhow::Error> {
    setup_gatex(app_config.clone(), changed).await?;
    Ok(())
}
