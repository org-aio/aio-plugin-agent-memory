use super::{
    MemoryError, Result, access,
    context::MemoryContext,
    spaces::{require_node_space, require_space},
    store,
};
use crate::service::MemoryService;
use axum::{
    Json,
    extract::{Path, Query, State},
};
use az_memory_model::{
    EdgeDraft, MemoryEdge, MemoryNode, NodeDraft, RollbackRequest, SearchRequest, WikiRevision,
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceQuery {
    pub space_id: Option<String>,
}

pub async fn create(
    State(service): State<Arc<MemoryService>>,
    Query(query): Query<SpaceQuery>,
    context: MemoryContext,
    Json(draft): Json<NodeDraft>,
) -> Result<(axum::http::StatusCode, Json<MemoryNode>)> {
    let mut transaction = service.pool.begin().await?;
    let space = require_space(
        &mut transaction,
        &context,
        query.space_id.as_deref(),
        true,
        false,
    )
    .await?;
    let node = store::save_node(
        &mut transaction,
        &space.id,
        &context.user_id,
        draft,
        None,
        "human",
        None,
    )
    .await?;
    transaction.commit().await?;
    Ok((axum::http::StatusCode::CREATED, Json(node)))
}

pub async fn get(
    State(service): State<Arc<MemoryService>>,
    Path(id): Path<String>,
    context: MemoryContext,
) -> Result<Json<MemoryNode>> {
    let mut transaction = service.pool.begin().await?;
    let space = require_node_space(&mut transaction, &context, &id, false).await?;
    let node = store::get_node(&mut transaction, &space.id, &id).await?;
    transaction.commit().await?;
    Ok(Json(node))
}

pub async fn update(
    State(service): State<Arc<MemoryService>>,
    Path(id): Path<String>,
    context: MemoryContext,
    Json(draft): Json<NodeDraft>,
) -> Result<Json<MemoryNode>> {
    let mut transaction = service.pool.begin().await?;
    let space = require_node_space(&mut transaction, &context, &id, true).await?;
    let node = store::save_node(
        &mut transaction,
        &space.id,
        &context.user_id,
        draft,
        Some(&id),
        "human",
        None,
    )
    .await?;
    transaction.commit().await?;
    Ok(Json(node))
}

pub async fn delete(
    State(service): State<Arc<MemoryService>>,
    Path(id): Path<String>,
    context: MemoryContext,
) -> Result<axum::http::StatusCode> {
    if context.worker() {
        return Err(MemoryError::Access);
    }
    let mut transaction = service.pool.begin().await?;
    let space = require_node_space(&mut transaction, &context, &id, true).await?;
    // 来源删除和后台整理使用同样的锁顺序，删除后旧任务不能继续提交。
    sqlx::query("SELECT id FROM plugin_memory_tasks WHERE id=$1 FOR UPDATE")
        .bind(&id)
        .fetch_optional(&mut *transaction)
        .await?;
    store::get_node(&mut transaction, &space.id, &id).await?;
    let source: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM plugin_memory_sources WHERE id=$1)")
            .bind(&id)
            .fetch_one(&mut *transaction)
            .await?;
    if source {
        sqlx::query("UPDATE plugin_memory_sources SET status='deleted',ciphertext=$2,updated_at=$3 WHERE id=$1")
            .bind(&id)
            .bind(Vec::<u8>::new())
            .bind(access::now())
            .execute(&mut *transaction)
            .await?;
        sqlx::query("UPDATE plugin_memory_nodes SET content='',title='已删除来源',version=version+1 WHERE id=$1")
            .bind(&id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM plugin_memory_secrets WHERE source_id=$1")
            .bind(&id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query(
            "UPDATE plugin_memory_tasks SET state='cancelled',lease=NULL,result=NULL WHERE id=$1",
        )
        .bind(&id)
        .execute(&mut *transaction)
        .await?;
        sqlx::query("DELETE FROM plugin_memory_revisions WHERE node_id=$1")
            .bind(&id)
            .execute(&mut *transaction)
            .await?;
    } else {
        sqlx::query("DELETE FROM plugin_memory_nodes WHERE id=$1")
            .bind(&id)
            .execute(&mut *transaction)
            .await?;
    }
    transaction.commit().await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn edges(
    State(service): State<Arc<MemoryService>>,
    Path(id): Path<String>,
    context: MemoryContext,
) -> Result<Json<Vec<MemoryEdge>>> {
    let mut transaction = service.pool.begin().await?;
    let space = require_node_space(&mut transaction, &context, &id, false).await?;
    store::get_node(&mut transaction, &space.id, &id).await?;
    let edges = store::links(&mut transaction, &space.id, &[id], false).await?;
    transaction.commit().await?;
    Ok(Json(edges))
}

pub async fn revisions(
    State(service): State<Arc<MemoryService>>,
    Path(id): Path<String>,
    context: MemoryContext,
) -> Result<Json<Vec<WikiRevision>>> {
    let mut transaction = service.pool.begin().await?;
    let space = require_node_space(&mut transaction, &context, &id, false).await?;
    let revisions = store::revisions(&mut transaction, &space.id, &id).await?;
    transaction.commit().await?;
    Ok(Json(revisions))
}

pub async fn rollback(
    State(service): State<Arc<MemoryService>>,
    Path(id): Path<String>,
    context: MemoryContext,
    Json(request): Json<RollbackRequest>,
) -> Result<Json<MemoryNode>> {
    let mut transaction = service.pool.begin().await?;
    let space = require_node_space(&mut transaction, &context, &id, true).await?;
    let current = store::get_node(&mut transaction, &space.id, &id).await?;
    if current.version != request.current_version {
        return Err(MemoryError::Stale);
    }
    let revision = store::revisions(&mut transaction, &space.id, &id)
        .await?
        .into_iter()
        .find(|revision| revision.version == request.version)
        .ok_or(MemoryError::Missing)?;
    let mut draft = revision.draft;
    draft.version = Some(current.version);
    let node = store::save_node(
        &mut transaction,
        &space.id,
        &context.user_id,
        draft,
        Some(&id),
        "human",
        None,
    )
    .await?;
    transaction.commit().await?;
    Ok(Json(node))
}

pub async fn sources(
    State(service): State<Arc<MemoryService>>,
    Path(id): Path<String>,
    context: MemoryContext,
) -> Result<Json<Vec<az_memory_model::SourceView>>> {
    let mut transaction = service.pool.begin().await?;
    let space = require_node_space(&mut transaction, &context, &id, false).await?;
    store::get_node(&mut transaction, &space.id, &id).await?;
    let ids = sqlx::query_scalar::<_, String>(
        "SELECT source_id FROM plugin_memory_evidence WHERE node_id=$1",
    )
    .bind(&id)
    .fetch_all(&mut *transaction)
    .await?;
    let mut sources = Vec::new();
    for source in ids {
        sources.push(super::intake::source_view(&mut transaction, &context, &source).await?);
    }
    transaction.commit().await?;
    Ok(Json(sources))
}

pub async fn create_edge(
    State(service): State<Arc<MemoryService>>,
    Query(query): Query<SpaceQuery>,
    context: MemoryContext,
    Json(draft): Json<EdgeDraft>,
) -> Result<(axum::http::StatusCode, Json<MemoryEdge>)> {
    let mut transaction = service.pool.begin().await?;
    let space = require_space(
        &mut transaction,
        &context,
        query.space_id.as_deref(),
        true,
        false,
    )
    .await?;
    let edge = store::save_edge(&mut transaction, &space.id, draft).await?;
    transaction.commit().await?;
    Ok((axum::http::StatusCode::CREATED, Json(edge)))
}

pub async fn delete_edge(
    State(service): State<Arc<MemoryService>>,
    Path(id): Path<String>,
    context: MemoryContext,
) -> Result<axum::http::StatusCode> {
    access::require_id(&id)?;
    let mut transaction = service.pool.begin().await?;
    let row = sqlx::query("SELECT source,target FROM plugin_memory_edges WHERE id=$1")
        .bind(&id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(MemoryError::Missing)?;
    let source: String = sqlx::Row::try_get(&row, 0)?;
    let target: String = sqlx::Row::try_get(&row, 1)?;
    let source_space = require_node_space(&mut transaction, &context, &source, true).await?;
    store::get_node(&mut transaction, &source_space.id, &target).await?;
    sqlx::query("DELETE FROM plugin_memory_edges WHERE id=$1")
        .bind(&id)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn search(
    State(service): State<Arc<MemoryService>>,
    Query(query): Query<SpaceQuery>,
    context: MemoryContext,
    Json(search): Json<SearchRequest>,
) -> Result<Json<az_memory_model::MemoryGraph>> {
    let mut transaction = service.pool.begin().await?;
    let space = require_space(
        &mut transaction,
        &context,
        query.space_id.as_deref(),
        false,
        false,
    )
    .await?;
    let graph = store::graph(&mut transaction, &space.id, search, true).await?;
    transaction.commit().await?;
    Ok(Json(graph))
}
