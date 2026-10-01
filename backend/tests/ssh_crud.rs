use anyhow::Result;
use axum::extract::Path;
use axum::{Json, Router, extract::State, routing::post};
use az_memory_model::SshHostDraft;
use az_memory_server::{
    cryptography::Cryptography,
    model::MemoryContext,
    service::{self, MemoryError, MemoryService},
};
use serde_json::{Value, json};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::{str::FromStr, sync::Arc};
use tokio::sync::Mutex;

#[derive(Clone)]
struct Host {
    tasks: Arc<Mutex<Vec<Value>>>,
    fail_remove: Arc<Mutex<bool>>,
    unavailable: Arc<Mutex<bool>>,
}

fn service_error(error: MemoryError) -> anyhow::Error {
    anyhow::anyhow!("{error:?}")
}

fn text(body: &Value, key: &str) -> String {
    body.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

async fn workers(State(host): State<Host>, Json(body): Json<Value>) -> Json<Value> {
    if *host.unavailable.lock().await {
        return Json(json!({"error":"设备不可用"}));
    }
    match text(&body, "operation").as_str() {
        "list" => Json(json!([{
            "id":"11111111-1111-4111-8111-111111111111",
            "label":"本机 Worker","platform":"macOS","status":"active"
        }])),
        "submit" => {
            let input = body.get("input").cloned().unwrap_or(Value::Null);
            let action = text(&input, "action");
            let failed = action == "remove" && *host.fail_remove.lock().await;
            host.tasks.lock().await.push(input.clone());
            let result = match action.as_str() {
                "ensure-key" => {
                    json!({"created":true,"path":"/home/.ssh/id_ed25519","mode":384,"publicKey":"ssh-ed25519 AAAAC3 fixture"})
                }
                "verify" => json!({"alias":"okm","verified":true,"stdout":""}),
                "remove" => json!({"alias":"okm","removed":true}),
                _ => json!({"alias":"okm","configured":true}),
            };
            Json(
                json!({"id": text(&body, "requestId"), "state": if failed {"failed"} else {"complete"}, "error": if failed {"remove 失败"} else {""}, "result": result}),
            )
        }
        "task" => Json(json!({"id": text(&body, "taskId"), "state":"complete"})),
        _ => Json(json!({"error":"不支持"})),
    }
}

#[tokio::test]
#[ignore = "needs local AIO_MEMORY_TEST_DATABASE_URL ending in _test"]
async fn ssh_hosts_apply_verify_and_remove_through_paired_device() -> Result<()> {
    let database_url = std::env::var("AIO_MEMORY_TEST_DATABASE_URL")?;
    let options = PgConnectOptions::from_str(&database_url)?;
    anyhow::ensure!(
        matches!(options.get_host(), "127.0.0.1" | "localhost")
            && options
                .get_database()
                .is_some_and(|name| name.ends_with("_test")),
        "只允许本机隔离测试数据库"
    );
    let admin = PgPoolOptions::new().connect_with(options.clone()).await?;
    let schema = format!("memory_test_{}", uuid::Uuid::new_v4().simple());
    sqlx::raw_sql(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await?;
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect_with(options.options([("search_path", schema.as_str())]))
        .await?;
    for sql in [
        include_str!("../migrations/0001_memory.sql"),
        include_str!("../migrations/0002_intake.sql"),
        include_str!("../migrations/0003_aliases.sql"),
        include_str!("../migrations/0004_source_versions.sql"),
        include_str!("../migrations/0005_recorded_queries.sql"),
        include_str!("../migrations/0006_ssh.sql"),
    ] {
        sqlx::raw_sql(sql).execute(&pool).await?;
    }

    let socket =
        std::env::temp_dir().join(format!("memory-ssh-{}.sock", uuid::Uuid::new_v4().simple()));
    let listener = tokio::net::UnixListener::bind(&socket)?;
    let host = Host {
        tasks: Arc::new(Mutex::new(Vec::new())),
        fail_remove: Arc::new(Mutex::new(false)),
        unavailable: Arc::new(Mutex::new(false)),
    };
    let app = Router::new()
        .route("/workers", post(workers))
        .with_state(host.clone());
    let broker = tokio::spawn(async move { axum::serve(listener, app).await });
    let service = Arc::new(MemoryService::new(
        pool.clone(),
        Cryptography::new(Some(socket.clone()), Some("test-only".into())),
        Some(socket.clone()),
        Some("test-only".into()),
    )?);

    let outcome = tokio::spawn(run_cases(service, host.clone())).await;
    pool.close().await;
    broker.abort();
    let _ = broker.await;
    std::fs::remove_file(socket)?;
    sqlx::raw_sql(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin)
        .await?;
    admin.close().await;
    outcome??;
    Ok(())
}

async fn run_cases(service: Arc<MemoryService>, host: Host) -> Result<()> {
    let owner = MemoryContext {
        tenant_id: "test".into(),
        user_id: "owner".into(),
        context_id: None,
    };
    let devices = service::ssh::devices(axum::extract::State(service.clone()), owner.clone())
        .await
        .map_err(service_error)?
        .0;
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].label, "本机 Worker");

    let created = service::ssh::create(
        axum::extract::State(service.clone()),
        owner.clone(),
        Json(SshHostDraft {
            alias: "okm".into(),
            hostname: "61.163.60.13".into(),
            user: "root".into(),
            port: 22,
            identity_file: "id_ed25519".into(),
            device_id: "11111111-1111-4111-8111-111111111111".into(),
            version: None,
        }),
    )
    .await
    .map_err(service_error)?
    .0;
    assert_eq!(created.status, "draft");
    assert_eq!(created.device_label, "本机 Worker");

    let applied = service::ssh::apply(
        axum::extract::State(service.clone()),
        Path(created.id.clone()),
        owner.clone(),
    )
    .await
    .map_err(service_error)?
    .0;
    assert_eq!(applied.host.status, "applied");
    assert!(applied.public_key.is_none());
    let upsert = host.tasks.lock().await.last().cloned().unwrap();
    assert_eq!(text(&upsert, "action"), "upsert");
    assert_eq!(text(&upsert, "identityFile"), "id_ed25519");

    let verified = service::ssh::verify(
        axum::extract::State(service.clone()),
        Path(created.id.clone()),
        owner.clone(),
    )
    .await
    .map_err(service_error)?
    .0;
    assert_eq!(verified.host.status, "verified");
    assert!(verified.public_key.is_some());
    let actions: Vec<String> = host
        .tasks
        .lock()
        .await
        .iter()
        .map(|task| text(task, "action"))
        .collect();
    assert_eq!(actions, ["upsert", "ensure-key", "verify"]);

    // 设备删除失败时必须保留记录并写入错误状态。
    *host.fail_remove.lock().await = true;
    let failure = service::ssh::delete(
        axum::extract::State(service.clone()),
        Path(created.id.clone()),
        owner.clone(),
    )
    .await;
    assert!(failure.is_err());
    let hosts = service::ssh::list(axum::extract::State(service.clone()), owner.clone())
        .await
        .map_err(service_error)?
        .0;
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0].status, "error");

    // 设备恢复后删除应同时移除记录。
    *host.fail_remove.lock().await = false;
    let status = service::ssh::delete(
        axum::extract::State(service.clone()),
        Path(created.id.clone()),
        owner.clone(),
    )
    .await
    .map_err(service_error)?;
    assert_eq!(status, axum::http::StatusCode::NO_CONTENT);
    let hosts = service::ssh::list(axum::extract::State(service.clone()), owner.clone())
        .await
        .map_err(service_error)?
        .0;
    assert!(hosts.is_empty());

    // 非法 alias 在写入前被拒绝。
    let invalid = service::ssh::create(
        axum::extract::State(service.clone()),
        owner.clone(),
        Json(SshHostDraft {
            alias: "bad;rm -rf".into(),
            hostname: "example.com".into(),
            user: "root".into(),
            port: 22,
            identity_file: "id_ed25519".into(),
            device_id: "11111111-1111-4111-8111-111111111111".into(),
            version: None,
        }),
    )
    .await;
    assert!(invalid.is_err());

    // 设备不可用时状态存储错误，不回传原始异常。
    *host.unavailable.lock().await = true;
    let unavailable = service::ssh::devices(axum::extract::State(service), owner).await;
    assert!(unavailable.is_err());
    Ok(())
}
