use anyhow::Result;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::post,
};
use az_memory_model::{CaptureRequest, SourceQuery, SourceUpdate, TaskFailure};
use az_memory_server::{
    cryptography::Cryptography,
    model::MemoryContext,
    service::{self, MemoryError, MemoryService},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::{collections::HashMap, str::FromStr, sync::Arc};
use tokio::sync::Mutex;

// 夹具模拟宿主 Keyring：数据库只保存随机票据，明文只驻留在测试进程内。
type Vault = Arc<Mutex<HashMap<String, (String, String)>>>;

fn service_error(error: MemoryError) -> anyhow::Error {
    anyhow::anyhow!("{error:?}")
}

#[derive(Deserialize)]
struct CryptoRequest {
    purpose: String,
    value: String,
}

async fn seal(State(vault): State<Vault>, Json(body): Json<CryptoRequest>) -> Json<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    vault
        .lock()
        .await
        .insert(id.clone(), (body.purpose, body.value));
    Json(json!({"value": STANDARD.encode(id)}))
}

async fn open(
    State(vault): State<Vault>,
    Json(body): Json<CryptoRequest>,
) -> std::result::Result<Json<Value>, axum::http::StatusCode> {
    use axum::http::StatusCode;
    let bytes = STANDARD
        .decode(&body.value)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let id = String::from_utf8(bytes).map_err(|_| StatusCode::BAD_REQUEST)?;
    let values = vault.lock().await;
    let (purpose, value) = values.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    if purpose != &body.purpose {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(Json(json!({"value":value})))
}

fn query(space: &str, text: &str, offset: i64, limit: i64) -> SourceQuery {
    SourceQuery {
        space_id: Some(space.into()),
        query: text.into(),
        status: String::new(),
        offset,
        limit,
    }
}

#[tokio::test]
#[ignore = "needs local AIO_MEMORY_TEST_DATABASE_URL ending in _test"]
async fn source_crud_preserves_isolation_permissions_versions_and_leases() -> Result<()> {
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
    ] {
        sqlx::raw_sql(sql).execute(&pool).await?;
    }
    let socket =
        std::env::temp_dir().join(format!("memory-{}.sock", uuid::Uuid::new_v4().simple()));
    let listener = tokio::net::UnixListener::bind(&socket)?;
    let app = Router::new()
        .route("/cryptography/seal", post(seal))
        .route("/cryptography/open", post(open))
        .with_state(Vault::default());
    let broker = tokio::spawn(async move { axum::serve(listener, app).await });
    let service = Arc::new(MemoryService::new(
        pool.clone(),
        Cryptography::new(Some(socket.clone()), Some("test-only".into())),
    ));
    // 子任务捕获断言失败，确保本次拥有的 schema 和 socket 仍能清理。
    let outcome = tokio::spawn(run_cases(service, pool.clone())).await;
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

async fn run_cases(service: Arc<MemoryService>, pool: sqlx::PgPool) -> Result<()> {
    let owner = MemoryContext {
        tenant_id: "test".into(),
        user_id: "owner".into(),
        context_id: None,
    };
    let spaces = service::spaces::list(State(service.clone()), owner.clone())
        .await
        .map_err(service_error)?
        .0;
    let space = &spaces[0].id;
    let raw = "# 读书笔记\n\npassword: test-canary\n\n- 第一条";
    let captured = service::intake::capture(
        State(service.clone()),
        owner.clone(),
        Json(CaptureRequest {
            request_id: "create-1".into(),
            text: raw.into(),
            space_id: Some(space.clone()),
            origin: "note".into(),
            reference: String::new(),
            clarifies: None,
        }),
    )
    .await
    .map_err(service_error)?
    .0;
    assert!(captured.can_edit && captured.can_delete);
    assert!(!captured.text.contains("test-canary"));
    let secret = &captured.secrets[0].id;
    let ciphertext: Vec<u8> =
        sqlx::query_scalar("SELECT ciphertext FROM plugin_memory_sources WHERE id=$1")
            .bind(&captured.id)
            .fetch_one(&pool)
            .await?;
    assert!(!String::from_utf8_lossy(&ciphertext).contains("test-canary"));

    sqlx::query("INSERT INTO plugin_memory_members(space_id,user_id,role) VALUES($1,'reader','READER'),($1,'editor','EDITOR')").bind(space).execute(&pool).await?;
    sqlx::query("INSERT INTO plugin_memory_secret_grants(secret_id,user_id,can_reveal,can_manage) VALUES($1,'reader',true,false)").bind(secret).execute(&pool).await?;
    for user in ["reader", "editor", "outsider"] {
        let context = MemoryContext {
            user_id: user.into(),
            ..owner.clone()
        };
        let result = service::source_edit::update(
            State(service.clone()),
            Path(captured.id.clone()),
            context,
            Json(SourceUpdate {
                text: "forbidden".into(),
                version: 1,
            }),
        )
        .await;
        assert!(matches!(result, Err(MemoryError::Access)));
    }
    let worker = MemoryContext {
        context_id: Some("service:fixture".into()),
        ..owner.clone()
    };
    assert!(matches!(
        service::source_edit::update(
            State(service.clone()),
            Path(captured.id.clone()),
            worker.clone(),
            Json(SourceUpdate {
                text: "worker".into(),
                version: 1
            })
        )
        .await,
        Err(MemoryError::Access)
    ));

    let old_lease = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    sqlx::query("UPDATE plugin_memory_tasks SET state='running',lease=$2,worker_id='owner',lease_until=9999999999999 WHERE id=$1").bind(&captured.id).bind(old_lease).execute(&pool).await?;
    let revised_raw = "# 修订的笔记\n\npassword: test-canary\n\n- 第二条";
    let updated = service::source_edit::update(
        State(service.clone()),
        Path(captured.id.clone()),
        owner.clone(),
        Json(SourceUpdate {
            text: revised_raw.into(),
            version: 1,
        }),
    )
    .await
    .map_err(service_error)?
    .0;
    assert_eq!(updated.version, 2);
    assert_eq!(&updated.secrets[0].id, secret);
    assert!(!updated.text.contains("test-canary"));
    let task: (String, Option<String>) =
        sqlx::query_as("SELECT state,lease FROM plugin_memory_tasks WHERE id=$1")
            .bind(&captured.id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(task, ("pending".into(), None));
    let grant: bool = sqlx::query_scalar("SELECT can_reveal FROM plugin_memory_secret_grants WHERE secret_id=$1 AND user_id='reader'").bind(secret).fetch_one(&pool).await?;
    assert!(grant);
    assert!(matches!(
        service::queue::fail(
            State(service.clone()),
            Path(captured.id.clone()),
            worker.clone(),
            Json(TaskFailure {
                lease: old_lease.into(),
                code: "invalid_result".into()
            })
        )
        .await,
        Err(MemoryError::Stale)
    ));
    assert_eq!(
        service::intake::original(
            State(service.clone()),
            Path(captured.id.clone()),
            owner.clone()
        )
        .await
        .map_err(service_error)?
        .0
        .value,
        revised_raw
    );
    let stale = service::source_edit::update(
        State(service.clone()),
        Path(captured.id.clone()),
        owner.clone(),
        Json(SourceUpdate {
            text: "旧窗口".into(),
            version: 1,
        }),
    )
    .await;
    assert!(matches!(stale, Err(MemoryError::Stale)));

    for context in [
        worker,
        MemoryContext {
            user_id: "reader".into(),
            ..owner.clone()
        },
    ] {
        assert!(matches!(
            service::nodes::delete(State(service.clone()), Path(captured.id.clone()), context)
                .await,
            Err(MemoryError::Access)
        ));
    }
    let replaced = service::source_edit::update(
        State(service.clone()),
        Path(captured.id.clone()),
        owner.clone(),
        Json(SourceUpdate {
            text: "修订后\npassword: replacement-canary".into(),
            version: 2,
        }),
    )
    .await
    .map_err(service_error)?
    .0;
    assert_eq!(replaced.version, 3);
    assert_ne!(&replaced.secrets[0].id, secret);
    assert!(!replaced.text.contains("replacement-canary"));
    let old_grants: i64 =
        sqlx::query_scalar("SELECT count(*) FROM plugin_memory_secret_grants WHERE secret_id=$1")
            .bind(secret)
            .fetch_one(&pool)
            .await?;
    assert_eq!(old_grants, 0);

    for n in 0..3 {
        let _ = service::intake::capture(
            State(service.clone()),
            owner.clone(),
            Json(CaptureRequest {
                request_id: format!("page-{n}"),
                text: format!("分页示例 {n}"),
                space_id: Some(space.clone()),
                origin: "note".into(),
                reference: String::new(),
                clarifies: None,
            }),
        )
        .await
        .map_err(service_error)?;
    }
    let first = service::intake::list(
        State(service.clone()),
        Query(query(space, "分页示例", 0, 2)),
        owner.clone(),
    )
    .await
    .map_err(service_error)?
    .0;
    let second = service::intake::list(
        State(service.clone()),
        Query(query(space, "分页示例", 2, 2)),
        owner.clone(),
    )
    .await
    .map_err(service_error)?
    .0;
    assert_eq!(first.total, 3);
    assert!(first.truncated);
    assert_eq!(second.sources.len(), 1);
    assert!(!second.truncated);
    assert!(first.sources.iter().all(|s| s.id != second.sources[0].id));
    assert_eq!(
        service::intake::list(
            State(service.clone()),
            Query(query(space, "%", 0, 2)),
            owner.clone()
        )
        .await
        .map_err(service_error)?
        .0
        .total,
        0
    );
    assert_eq!(
        service::intake::list(
            State(service.clone()),
            Query(query(space, "test-canary", 0, 2)),
            owner.clone()
        )
        .await
        .map_err(service_error)?
        .0
        .total,
        0
    );

    service::nodes::delete(
        State(service.clone()),
        Path(captured.id.clone()),
        owner.clone(),
    )
    .await
    .map_err(service_error)?;
    assert!(matches!(
        service::intake::get(
            State(service.clone()),
            Path(captured.id.clone()),
            owner.clone()
        )
        .await,
        Err(MemoryError::Missing)
    ));
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM plugin_memory_secrets WHERE source_id=$1")
            .bind(&captured.id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(count, 0);
    assert_eq!(
        service::intake::list(
            State(service.clone()),
            Query(query(space, "修订", 0, 24)),
            owner
        )
        .await
        .map_err(service_error)?
        .0
        .total,
        0
    );
    Ok(())
}
