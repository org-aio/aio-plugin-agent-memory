use anyhow::{Context, Result, ensure};
use az_plugin_contract::process::Configuration;
use std::path::PathBuf;

#[derive(Clone)]
pub struct RuntimeConfig {
    pub database_url: String,
    pub broker_socket: Option<PathBuf>,
    pub broker_token: Option<String>,
    /// 宿主提供的匿名公开路由前缀；本地开发可用环境变量覆盖。
    pub public_base: String,
}

impl RuntimeConfig {
    pub fn from_host(host: Configuration) -> Result<Self> {
        ensure!(
            host.abi_version == 2
                && !host.tenant_id.is_empty()
                && host.ingress_token.len() >= 32
                && PathBuf::from(&host.broker_socket).is_absolute(),
            "宿主绑定无效"
        );
        Ok(Self {
            database_url: host.database_url.context("宿主未授权数据库")?,
            broker_socket: Some(host.broker_socket.into()),
            broker_token: Some(host.ingress_token),
            public_base: validate_public_base(host.public_base)?,
        })
    }

    pub fn from_env() -> Result<Self> {
        Ok(Self {
            database_url: std::env::var("AIO_MEMORY_DATABASE_URL")
                .context("缺少插件专属数据库连接")?,
            broker_socket: std::env::var_os("AIO_MEMORY_BROKER_SOCKET").map(PathBuf::from),
            broker_token: std::env::var("AIO_MEMORY_BROKER_TOKEN").ok(),
            public_base: validate_public_base(std::env::var("AIO_MEMORY_PUBLIC_BASE").ok())?,
        })
    }
}

/// 只接受宿主生成的站内绝对前缀；不能带查询、片段、尾斜杠或路径归一化。
fn validate_public_base(value: Option<String>) -> Result<String> {
    let Some(value) = value else {
        return Ok(String::new());
    };
    ensure!(
        value.is_empty()
            || (value.starts_with('/')
                && !value.ends_with('/')
                && value.len() <= 512
                && !value.split('/').any(|part| matches!(part, "." | ".."))
                && !value.chars().any(char::is_control)
                && !value.contains('?')
                && !value.contains('#')),
        "公开分享基址无效"
    );
    Ok(value)
}
