use anyhow::{Context, Result, ensure};
use az_plugin_contract::process::Configuration;
use std::path::PathBuf;

#[derive(Clone)]
pub struct RuntimeConfig {
    pub database_url: String,
    pub broker_socket: Option<PathBuf>,
    pub broker_token: Option<String>,
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
        })
    }

    pub fn from_env() -> Result<Self> {
        Ok(Self {
            database_url: std::env::var("AIO_MEMORY_DATABASE_URL")
                .context("缺少插件专属数据库连接")?,
            broker_socket: std::env::var_os("AIO_MEMORY_BROKER_SOCKET").map(PathBuf::from),
            broker_token: std::env::var("AIO_MEMORY_BROKER_TOKEN").ok(),
        })
    }
}
