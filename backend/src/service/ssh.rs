use super::{MemoryError, Result, context::MemoryContext};
use crate::service::MemoryService;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use az_memory_model::{SshApplyResult, SshDevice, SshHost, SshHostDraft};
use reqwest::Client;
use serde_json::{Value, json};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

const CAPABILITY: &str = "ssh.manage";

pub async fn list(
    State(service): State<Arc<MemoryService>>,
    context: MemoryContext,
) -> Result<Json<Vec<SshHost>>> {
    if context.worker() {
        return Err(MemoryError::Access);
    }
    let rows = sqlx::query(SELECT)
        .bind(&context.tenant_id)
        .bind(&context.user_id)
        .fetch_all(&service.pool)
        .await?;
    rows.into_iter()
        .map(host)
        .collect::<Result<Vec<_>>>()
        .map(Json)
}

pub async fn create(
    State(service): State<Arc<MemoryService>>,
    context: MemoryContext,
    Json(draft): Json<SshHostDraft>,
) -> Result<Json<SshHost>> {
    require_editor(&context)?;
    let draft = validate(draft)?;
    let label = device_label(&service, &context, &draft.device_id).await?;
    let id = super::access::id();
    let now = super::access::now();
    sqlx::query("INSERT INTO plugin_memory_ssh_hosts(id,tenant_id,user_id,alias,hostname,username,port,identity_file,device_id,device_label,status,version,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,'draft',1,$11)")
        .bind(&id)
        .bind(&context.tenant_id)
        .bind(&context.user_id)
        .bind(&draft.alias)
        .bind(&draft.hostname)
        .bind(&draft.user)
        .bind(draft.port as i32)
        .bind(&draft.identity_file)
        .bind(&draft.device_id)
        .bind(&label)
        .bind(now)
        .execute(&service.pool)
        .await
        .map_err(unique_error)?;
    load(&service, &context, &id).await.map(Json)
}

pub async fn update(
    State(service): State<Arc<MemoryService>>,
    Path(id): Path<String>,
    context: MemoryContext,
    Json(draft): Json<SshHostDraft>,
) -> Result<Json<SshHost>> {
    require_editor(&context)?;
    super::access::require_id(&id)?;
    let draft = validate(draft)?;
    let version = draft
        .version
        .ok_or_else(|| MemoryError::Input("缺少版本".into()))?;
    let label = device_label(&service, &context, &draft.device_id).await?;
    let changed = sqlx::query("UPDATE plugin_memory_ssh_hosts SET alias=$1,hostname=$2,username=$3,port=$4,identity_file=$5,device_id=$6,device_label=$7,status='draft',last_error=NULL,version=version+1,updated_at=$8 WHERE id=$9 AND tenant_id=$10 AND user_id=$11 AND version=$12")
        .bind(&draft.alias)
        .bind(&draft.hostname)
        .bind(&draft.user)
        .bind(draft.port as i32)
        .bind(&draft.identity_file)
        .bind(&draft.device_id)
        .bind(&label)
        .bind(super::access::now())
        .bind(&id)
        .bind(&context.tenant_id)
        .bind(&context.user_id)
        .bind(version)
        .execute(&service.pool)
        .await
        .map_err(unique_error)?
        .rows_affected();
    if changed != 1 {
        return Err(MemoryError::Stale);
    }
    load(&service, &context, &id).await.map(Json)
}

pub async fn delete(
    State(service): State<Arc<MemoryService>>,
    Path(id): Path<String>,
    context: MemoryContext,
) -> Result<StatusCode> {
    require_editor(&context)?;
    let host = load(&service, &context, &id).await?;
    let task = broker(&service)
        .submit(
            &context,
            &host.device_id,
            json!({"action":"remove","alias":host.alias}),
            task_id(),
        )
        .await?;
    if !task_success(&task) {
        let error = task_error(&task);
        set_status(&service, &host, "error", Some(&error)).await?;
        return Err(MemoryError::Input(error));
    }
    let changed = sqlx::query("DELETE FROM plugin_memory_ssh_hosts WHERE id=$1 AND tenant_id=$2 AND user_id=$3 AND version=$4")
        .bind(&id)
        .bind(&context.tenant_id)
        .bind(&context.user_id)
        .bind(host.version)
        .execute(&service.pool)
        .await?
        .rows_affected();
    if changed != 1 {
        return Err(MemoryError::Stale);
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn devices(
    State(service): State<Arc<MemoryService>>,
    context: MemoryContext,
) -> Result<Json<Vec<SshDevice>>> {
    require_editor(&context)?;
    let value = broker(&service)
        .request(
            &context,
            json!({"operation":"list","capability":CAPABILITY}),
        )
        .await?;
    let rows = value
        .as_array()
        .ok_or_else(|| MemoryError::storage(anyhow::anyhow!("设备列表格式无效")))?;
    let devices = rows
        .iter()
        .filter_map(|row| {
            Some(SshDevice {
                id: row.get("id")?.as_str()?.to_owned(),
                label: row.get("label")?.as_str()?.to_owned(),
                platform: row.get("platform")?.as_str()?.to_owned(),
                status: row.get("status")?.as_str()?.to_owned(),
                last_seen: row.get("last_seen").and_then(Value::as_i64),
            })
        })
        .collect();
    Ok(Json(devices))
}

pub async fn apply(
    State(service): State<Arc<MemoryService>>,
    Path(id): Path<String>,
    context: MemoryContext,
) -> Result<Json<SshApplyResult>> {
    require_editor(&context)?;
    let host = load(&service, &context, &id).await?;
    let task = broker(&service)
        .submit(
            &context,
            &host.device_id,
            json!({"action":"upsert","alias":host.alias,"hostname":host.hostname,"user":host.user,"port":host.port,"identityFile":host.identity_file.replace(".pub", "")}),
            task_id(),
        )
        .await?;
    let success = task_success(&task);
    let message = if success {
        "已写入配对设备".to_owned()
    } else {
        task_error(&task)
    };
    let status = if success { "applied" } else { "error" };
    set_status(
        &service,
        &host,
        status,
        if success { None } else { Some(&message) },
    )
    .await?;
    let host = load(&service, &context, &id).await?;
    Ok(Json(SshApplyResult {
        host,
        message,
        public_key: None,
    }))
}

pub async fn verify(
    State(service): State<Arc<MemoryService>>,
    Path(id): Path<String>,
    context: MemoryContext,
) -> Result<Json<SshApplyResult>> {
    require_editor(&context)?;
    let host = load(&service, &context, &id).await?;
    let key = broker(&service)
        .submit(
            &context,
            &host.device_id,
            json!({"action":"ensure-key"}),
            task_id(),
        )
        .await?;
    if !task_success(&key) {
        return Ok(Json(SshApplyResult {
            host,
            message: task_error(&key),
            public_key: None,
        }));
    }
    let public_key = key
        .pointer("/result/publicKey")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let task = broker(&service)
        .submit(
            &context,
            &host.device_id,
            json!({"action":"verify","alias":host.alias,"hostname":host.hostname,"user":host.user,"port":host.port,"identityFile":host.identity_file}),
            task_id(),
        )
        .await?;
    let success = task_success(&task);
    let message = if success {
        "免密连接验证通过".to_owned()
    } else {
        task_error(&task)
    };
    let status = if success { "verified" } else { "error" };
    set_status(
        &service,
        &host,
        status,
        if success { None } else { Some(&message) },
    )
    .await?;
    let host = load(&service, &context, &id).await?;
    Ok(Json(SshApplyResult {
        host,
        message,
        public_key,
    }))
}

async fn device_label(
    service: &MemoryService,
    context: &MemoryContext,
    device_id: &str,
) -> Result<String> {
    let value = broker(service)
        .request(context, json!({"operation":"list","capability":CAPABILITY}))
        .await?;
    let label = value.as_array().and_then(|rows| {
        rows.iter()
            .find(|row| row.get("id").and_then(Value::as_str) == Some(device_id))
            .and_then(|row| row.get("label").and_then(Value::as_str))
            .map(str::to_owned)
    });
    Ok(label.unwrap_or_default())
}

async fn load(service: &MemoryService, context: &MemoryContext, id: &str) -> Result<SshHost> {
    super::access::require_id(id)?;
    let row = sqlx::query(&format!("{SELECT} AND id=$3"))
        .bind(&context.tenant_id)
        .bind(&context.user_id)
        .bind(id)
        .fetch_optional(&service.pool)
        .await?
        .ok_or(MemoryError::Missing)?;
    host(row)
}

async fn set_status(
    service: &MemoryService,
    host: &SshHost,
    status: &str,
    error: Option<&str>,
) -> Result<()> {
    let changed = sqlx::query("UPDATE plugin_memory_ssh_hosts SET status=$1,last_error=$2,version=version+1,updated_at=$3 WHERE id=$4 AND version=$5")
        .bind(status)
        .bind(error)
        .bind(super::access::now())
        .bind(&host.id)
        .bind(host.version)
        .execute(&service.pool)
        .await?
        .rows_affected();
    if changed != 1 {
        return Err(MemoryError::Stale);
    }
    Ok(())
}

fn host(row: sqlx::postgres::PgRow) -> Result<SshHost> {
    Ok(SshHost {
        id: row.try_get(0)?,
        alias: row.try_get(1)?,
        hostname: row.try_get(2)?,
        user: row.try_get(3)?,
        port: u16::try_from(row.try_get::<i32, _>(4)?)
            .map_err(|_| MemoryError::storage(anyhow::anyhow!("SSH 端口无效")))?,
        identity_file: row.try_get(5)?,
        device_id: row.try_get(6)?,
        device_label: row.try_get(7)?,
        status: row.try_get(8)?,
        last_error: row.try_get(9)?,
        version: row.try_get(10)?,
        updated_at: row.try_get(11)?,
    })
}

fn require_editor(context: &MemoryContext) -> Result<()> {
    if context.worker() || context.user_id.is_empty() {
        Err(MemoryError::Access)
    } else {
        Ok(())
    }
}

fn validate(mut draft: SshHostDraft) -> Result<SshHostDraft> {
    draft.alias = draft.alias.trim().to_owned();
    draft.hostname = draft.hostname.trim().to_owned();
    draft.user = draft.user.trim().to_owned();
    draft.identity_file = draft.identity_file.trim().to_owned();
    if !alias(&draft.alias) {
        return Err(MemoryError::Input(
            "SSH 别名只能包含字母、数字、点、下划线和连字符".into(),
        ));
    }
    if !hostname(&draft.hostname) {
        return Err(MemoryError::Input("SSH 主机名无效".into()));
    }
    if !user(&draft.user) {
        return Err(MemoryError::Input("SSH 用户名无效".into()));
    }
    if !(1..=65535).contains(&draft.port) {
        return Err(MemoryError::Input("SSH 端口无效".into()));
    }
    if !identity(&draft.identity_file) {
        return Err(MemoryError::Input("SSH 身份文件无效".into()));
    }
    Uuid::parse_str(&draft.device_id).map_err(|_| MemoryError::Input("设备 ID 无效".into()))?;
    Ok(draft)
}

fn alias(value: &str) -> bool {
    let mut bytes = value.bytes();
    value.len() <= 63
        && bytes
            .next()
            .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}

fn hostname(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 253
        && value.split('.').all(|part| {
            !part.is_empty()
                && !part.starts_with('-')
                && !part.ends_with('-')
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

fn user(value: &str) -> bool {
    alias(value)
}

fn identity(value: &str) -> bool {
    matches!(value, "id_ed25519" | "id_rsa" | "id_ecdsa")
}

fn unique_error(error: sqlx::Error) -> MemoryError {
    if error
        .as_database_error()
        .is_some_and(|error| error.is_unique_violation())
    {
        MemoryError::Input("该 SSH 别名已存在".into())
    } else {
        MemoryError::from(error)
    }
}

fn task_id() -> String {
    Uuid::new_v4().to_string()
}

fn task_success(task: &Value) -> bool {
    task["state"].as_str() == Some("complete")
}

fn task_error(task: &Value) -> String {
    task["error"]
        .as_str()
        .or_else(|| task["message"].as_str())
        .unwrap_or("设备未完成 SSH 操作")
        .chars()
        .take(240)
        .collect()
}

const SELECT: &str = "SELECT id,alias,hostname,username,port,identity_file,device_id,device_label,status,last_error,version,updated_at FROM plugin_memory_ssh_hosts WHERE tenant_id=$1 AND user_id=$2";

struct Broker {
    client: Client,
    token: String,
}

impl Broker {
    async fn request(&self, context: &MemoryContext, body: Value) -> Result<Value> {
        let mut body = body;
        body["tenantId"] = json!(context.tenant_id);
        body["userId"] = json!(context.user_id);
        let response = self
            .client
            .post("http://localhost/workers")
            .header("x-aio-token", &self.token)
            .json(&body)
            .send()
            .await
            .map_err(|error| {
                MemoryError::storage(anyhow::Error::new(error).context("配对设备不可用"))
            })?;
        if !response.status().is_success() {
            return Err(MemoryError::storage(anyhow::anyhow!(
                "配对设备不可用或调用未授权"
            )));
        }
        response.json().await.map_err(|error| {
            MemoryError::storage(anyhow::Error::new(error).context("设备响应无效"))
        })
    }

    async fn submit(
        &self,
        context: &MemoryContext,
        device_id: &str,
        input: Value,
        request_id: String,
    ) -> Result<Value> {
        let task = self
            .request(
                context,
                json!({
                    "operation":"submit",
                    "capability":CAPABILITY,
                    "workerId":device_id,
                    "requestId":request_id,
                    "input":input
                }),
            )
            .await?;
        let id = task["id"]
            .as_str()
            .ok_or_else(|| MemoryError::storage(anyhow::anyhow!("设备任务 ID 缺失")))?
            .to_owned();
        self.wait(context, task, &id).await
    }

    async fn wait(&self, context: &MemoryContext, mut task: Value, id: &str) -> Result<Value> {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(25);
        loop {
            let state = task["state"].as_str().unwrap_or("");
            if matches!(state, "complete" | "failed" | "cancelled" | "interrupted") {
                return Ok(task);
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(MemoryError::Input(
                    "等待设备回报超时，执行结果待确认".into(),
                ));
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            task = self
                .request(
                    context,
                    json!({"operation":"task","capability":CAPABILITY,"taskId":id}),
                )
                .await?;
            if task["id"].as_str() != Some(id) {
                return Err(MemoryError::storage(anyhow::anyhow!("设备任务 ID 不匹配")));
            }
        }
    }
}

fn broker(service: &MemoryService) -> Broker {
    Broker {
        client: service.broker.clone(),
        token: service.broker_token.clone().unwrap_or_default(),
    }
}
