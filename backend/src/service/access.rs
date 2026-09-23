use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use uuid::Uuid;

pub type Result<T> = std::result::Result<T, MemoryError>;

#[derive(Debug)]
pub enum MemoryError {
    Input(String),
    Access,
    Missing,
    Stale,
    Storage(anyhow::Error),
}

impl MemoryError {
    pub fn storage(error: impl Into<anyhow::Error>) -> Self {
        Self::Storage(error.into())
    }
}

impl From<sqlx::Error> for MemoryError {
    fn from(error: sqlx::Error) -> Self {
        Self::Storage(error.into())
    }
}

impl From<anyhow::Error> for MemoryError {
    fn from(error: anyhow::Error) -> Self {
        Self::Storage(error)
    }
}

impl From<serde_json::Error> for MemoryError {
    fn from(error: serde_json::Error) -> Self {
        Self::Storage(error.into())
    }
}

#[derive(Serialize)]
struct Failure {
    error: String,
}

impl IntoResponse for MemoryError {
    fn into_response(self) -> Response {
        let (status, error) = match self {
            Self::Input(message) => (StatusCode::BAD_REQUEST, message),
            Self::Access => (StatusCode::FORBIDDEN, "没有访问权限".into()),
            Self::Missing => (StatusCode::NOT_FOUND, "记录不存在或已被删除".into()),
            Self::Stale => (
                StatusCode::CONFLICT,
                "内容已被其他会话修改，请刷新后重试".into(),
            ),
            Self::Storage(error) => {
                eprintln!("Memory 请求失败，存储事务已回滚: {error:#}");
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "存储暂不可用，请稍后重试".into(),
                )
            }
        };
        (status, Json(Failure { error })).into_response()
    }
}

pub fn id() -> String {
    Uuid::new_v4().simple().to_string()
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

pub fn require_id(value: &str) -> Result<()> {
    if value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(MemoryError::Input("节点或关系 ID 无效".into()))
    }
}
