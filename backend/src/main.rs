use anyhow::Result;
use az_memory_server::{
    configuration::RuntimeConfig,
    transport::{Ingress, router},
};

#[tokio::main]
async fn main() -> Result<()> {
    let (config, ingress) = match az_memory_server::hosting::load()? {
        Some(hosted) => hosted,
        None => {
            let token = std::env::var("AIO_MEMORY_INGRESS_TOKEN")?;
            anyhow::ensure!(token.len() >= 32, "入口票据至少 32 字节");
            (
                RuntimeConfig::from_env()?,
                Ingress {
                    token,
                    tenant: None,
                },
            )
        }
    };
    let service = az_memory_server::service::build(config).await?;
    let app = router(service.clone(), ingress);
    if let Some(path) = std::env::var_os("AIO_PLUGIN_SOCKET") {
        use std::os::unix::fs::PermissionsExt;
        if tokio::fs::try_exists(&path).await? {
            tokio::fs::remove_file(&path).await?;
        }
        let listener = tokio::net::UnixListener::bind(&path)?;
        tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o666)).await?;
        axum::serve(listener, app).await?;
        return Ok(());
    }
    let port = std::env::var("AIO_PLUGIN_PORT")
        .unwrap_or_else(|_| "4192".into())
        .parse::<u16>()?;
    let bind = std::env::var("AIO_MEMORY_BIND")
        .unwrap_or_else(|_| "127.0.0.1".into())
        .parse::<std::net::IpAddr>()?;
    let listener = tokio::net::TcpListener::bind((bind, port)).await?;
    println!("Memory backend listening on {bind}:{port}");
    axum::serve(listener, app).await?;
    Ok(())
}
