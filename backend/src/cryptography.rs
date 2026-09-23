use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use std::{io, path::PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Clone)]
pub struct Cryptography {
    socket: Option<PathBuf>,
    token: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct Request<'a> {
    purpose: &'a str,
    value: String,
}

#[derive(Deserialize)]
struct Response {
    value: String,
}

impl Cryptography {
    pub fn new(socket: Option<PathBuf>, token: Option<String>) -> Self {
        Self { socket, token }
    }

    pub async fn seal(&self, purpose: &str, value: &[u8]) -> Result<Vec<u8>> {
        self.call("seal", purpose, STANDARD.encode(value)).await
    }

    pub async fn open(&self, purpose: &str, value: &[u8]) -> Result<Vec<u8>> {
        self.call("open", purpose, STANDARD.encode(value)).await
    }

    async fn call(&self, operation: &str, purpose: &str, value: String) -> Result<Vec<u8>> {
        ensure!(!purpose.is_empty() && purpose.len() <= 512, "加密用途无效");
        let socket = self.socket.as_ref().context("宿主加密通道未绑定")?;
        let token = self.token.as_ref().context("宿主加密票据缺失")?;
        let request = Request { purpose, value };
        let body = serde_json::to_vec(&request)?;
        let response =
            broker_unix(socket, token, &format!("/cryptography/{operation}"), &body).await?;
        let response: Response = serde_json::from_slice(&response)?;
        STANDARD.decode(response.value).context("宿主加密响应无效")
    }
}

async fn broker_unix(
    socket: &std::path::Path,
    token: &str,
    path: &str,
    body: &[u8],
) -> Result<Vec<u8>> {
    let mut stream = tokio::net::UnixStream::connect(socket).await?;
    let request = format!(
        "POST {path} HTTP/1.1\r\nhost: localhost\r\ncontent-type: application/json\r\ncontent-length: {}\r\nx-aio-token: {token}\r\nconnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(request.as_bytes()).await?;
    stream.write_all(body).await?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await?;
    let split = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "broker 响应无效"))?;
    let headers = String::from_utf8_lossy(&response[..split]);
    ensure!(
        headers.starts_with("HTTP/1.1 200") || headers.starts_with("HTTP/1.0 200"),
        "broker 拒绝加密请求"
    );
    Ok(response[split + 4..].to_vec())
}
