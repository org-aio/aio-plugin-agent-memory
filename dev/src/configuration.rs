use anyhow::{Context, Result, ensure};
use az_plugin_contract::InvocationScope;
use az_plugin_runtime::{
    HostServices, Keyring,
    bindings::aio::plugin::transport::{Request, Response},
};
use std::{collections::BTreeMap, io::Write, path::Path};

pub fn load(root: &Path, isolated: bool) -> Result<(String, Keyring)> {
    let directory = std::env::var("AIO_MEMORY_DEV_DIRECTORY")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| root.join(".local"));
    if isolated {
        let mut key = [0u8; 32];
        getrandom::fill(&mut key).map_err(|_| anyhow::anyhow!("开发密钥随机源不可用"))?;
        return Ok((
            format!("memory-test-{}", uuid::Uuid::new_v4()),
            Keyring::new(
                "development".into(),
                BTreeMap::from([("development".into(), key)]),
            )?,
        ));
    }
    std::fs::create_dir_all(&directory)?;
    let path = directory.join("memory-host.json");
    if !path.exists() {
        let mut key = [0u8; 32];
        getrandom::fill(&mut key).map_err(|_| anyhow::anyhow!("开发密钥随机源不可用"))?;
        let value = serde_json::json!({"source":uuid::Uuid::new_v4().to_string(),"key":key.iter().map(|byte|format!("{byte:02x}")).collect::<String>()});
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options
            .open(&path)?
            .write_all(serde_json::to_string(&value)?.as_bytes())?;
    }
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
    let text = value["key"].as_str().context("开发宿主密钥不可读")?;
    ensure!(text.len() == 64 && text.is_ascii(), "开发宿主密钥无效");
    let bytes: Vec<u8> = (0..64)
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16))
        .collect::<std::result::Result<_, _>>()?;
    let key: [u8; 32] = bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("开发密钥无效"))?;
    Ok((
        value["source"]
            .as_str()
            .context("开发工作区身份不可读")?
            .into(),
        Keyring::new(
            "development".into(),
            BTreeMap::from([("development".into(), key)]),
        )?,
    ))
}

pub struct DevelopmentServices;

#[async_trait::async_trait]
impl HostServices for DevelopmentServices {
    async fn authorize(&self, scope: &InvocationScope, permission: &str) -> Result<bool> {
        Ok(permission == "memory:compile"
            && scope.context.session_id.as_deref() == Some("service:agent"))
    }
    async fn manage(&self, _: &InvocationScope, _: Request) -> Result<Response> {
        anyhow::bail!("开发宿主未开放插件管理")
    }
}
