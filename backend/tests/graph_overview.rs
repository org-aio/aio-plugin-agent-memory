use anyhow::Result;
use axum::{
    Json, Router,
    extract::{Query, State},
    routing::post,
};
use az_memory_model::{CaptureRequest, MemoryGraph, NodeDraft, NodeKind};
use az_memory_server::{
    cryptography::Cryptography,
    model::MemoryContext,
    service::{self, MemoryError, MemoryService},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::{collections::HashMap, str::FromStr, sync::Arc};
use tokio::sync::Mutex;

// 夹具模拟宿主 Keyring：数据库只保存随机票据，明文只驻留在测试进程内。
type Vault = Arc<Mutex<HashMap<String, (String, String)>>>;

fn service_error(error: MemoryError) -> anyhow::Error {
    anyhow::anyhow!("{error:?}")
}

#[derive(serde::Deserialize)]
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

/// 图谱概览必须忽略收件来源，只返回知识节点和它们之间的关系。
///
/// 真实空间里收件来源远多于知识条目；如果按更新时间取样，图谱会被来源淹没成
/// “有节点、零关系”的标签云。这里直接构造该场景：先写入大量来源，再写入少量
/// 带关系的知识条目，断言概览仍然完整给出知识节点和关系。
#[tokio::test]
#[ignore = "needs local AIO_MEMORY_TEST_DATABASE_URL ending in _test"]
async fn graph_overview_keeps_knowledge_relations_despite_many_sources() -> Result<()> {
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
    let schema = format!("memory_graph_{}", uuid::Uuid::new_v4().simple());
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
        include_str!("../migrations/0007_attachments.sql"),
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
        None,
        None,
    )?);
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

async fn run_cases(service: Arc<MemoryService>, _pool: sqlx::PgPool) -> Result<()> {
    let owner = MemoryContext {
        tenant_id: "test".into(),
        user_id: "owner".into(),
        context_id: None,
    };
    let spaces = service::spaces::list(State(service.clone()), owner.clone())
        .await
        .map_err(service_error)?
        .0;
    let space = spaces[0].id.clone();
    // 先写入 260 条来源，全部比知识条目更新，模拟真实收件积压。
    for index in 0..260 {
        let _ = service::intake::capture(
            State(service.clone()),
            owner.clone(),
            Json(CaptureRequest {
                request_id: format!("source-{index}"),
                text: format!("第 {index} 条收件资料"),
                space_id: Some(space.clone()),
                origin: "note".into(),
                reference: String::new(),
                clarifies: None,
                images: Vec::new(),
            }),
        )
        .await
        .map_err(service_error)?;
    }
    let first = service::nodes::create(
        State(service.clone()),
        Query(service::nodes::SpaceQuery {
            space_id: Some(space.clone()),
        }),
        owner.clone(),
        Json(NodeDraft {
            title: "甲项目".into(),
            kind: NodeKind::Project,
            content: "甲项目正文".into(),
            url: String::new(),
            tags: Vec::new(),
            version: None,
            aliases: Vec::new(),
        }),
    )
    .await
    .map_err(service_error)?
    .1
    .0;
    let second = service::nodes::create(
        State(service.clone()),
        Query(service::nodes::SpaceQuery {
            space_id: Some(space.clone()),
        }),
        owner.clone(),
        Json(NodeDraft {
            title: "乙人物".into(),
            kind: NodeKind::Person,
            content: "乙人物正文".into(),
            url: String::new(),
            tags: Vec::new(),
            version: None,
            aliases: Vec::new(),
        }),
    )
    .await
    .map_err(service_error)?
    .1
    .0;
    let _ = service::nodes::create_edge(
        State(service.clone()),
        Query(service::nodes::SpaceQuery {
            space_id: Some(space.clone()),
        }),
        owner.clone(),
        Json(az_memory_model::EdgeDraft {
            source: first.id.clone(),
            target: second.id.clone(),
            relation: "负责".into(),
            evidence: "验收依据".into(),
        }),
    )
    .await
    .map_err(service_error)?;

    let graph: MemoryGraph = service::graph::graph(
        State(service.clone()),
        Query(service::nodes::SpaceQuery {
            space_id: Some(space.clone()),
        }),
        owner.clone(),
    )
    .await
    .map_err(service_error)?
    .0;

    assert!(
        graph.nodes.iter().all(|node| node.kind != NodeKind::Source),
        "图谱概览不应包含收件来源"
    );
    assert!(
        graph.nodes.iter().any(|node| node.id == first.id),
        "较旧的知识条目也必须出现在图谱概览中"
    );
    assert_eq!(graph.total, 2, "概览总数应只统计知识节点");
    assert_eq!(graph.edges.len(), 1, "知识关系不能被来源挤出结果");
    assert_eq!(graph.edges[0].relation, "负责");
    // 来源仍可通过搜索和节点详情访问，只是不进入概览。
    let search = service::graph::search(
        State(service.clone()),
        Query(service::nodes::SpaceQuery {
            space_id: Some(space.clone()),
        }),
        owner.clone(),
        Json(az_memory_model::SearchRequest {
            query: "收件资料".into(),
            kind: None,
            limit: 24,
        }),
    )
    .await
    .map_err(service_error)?
    .0;
    assert!(
        search
            .nodes
            .iter()
            .any(|node| node.kind == NodeKind::Source),
        "关键词检索仍应能命中来源"
    );
    Ok(())
}
