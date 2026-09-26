use anyhow::{Context, Result, ensure};
use az_plugin_contract::process::Configuration;

use crate::{configuration::RuntimeConfig, transport::Ingress};

pub fn load() -> Result<Option<(RuntimeConfig, Ingress)>> {
    let Some(path) = std::env::var_os("AIO_PLUGIN_CONFIG") else {
        return Ok(None);
    };
    let metadata = std::fs::metadata(&path)?;
    ensure!(
        metadata.is_file() && metadata.len() <= 65_536,
        "宿主配置无效"
    );
    let host: Configuration =
        serde_json::from_slice(&std::fs::read(path)?).context("宿主配置协议无效")?;
    let ingress = Ingress {
        token: host.ingress_token.clone(),
        tenant: Some(host.tenant_id.clone()),
    };
    Ok(Some((RuntimeConfig::from_host(host)?, ingress)))
}

pub async fn describe() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "label": "智能体记忆",
        "pages": [{
            "id": "agent-memory",
            "label": "记忆",
            "entry": "index.html",
            "scene": ["workspace", "工作空间"],
            "menu_path": ["智能体"],
            "permission": null,
            "surface": "workspace"
        }]
    }))
}
