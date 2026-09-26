use super::{
    MemoryError, Result, access,
    context::MemoryContext,
    isolate::isolate,
    secrets,
    spaces::{require_node_space, require_space},
    store,
};
use crate::service::MemoryService;
use axum::{
    Json,
    extract::{Path, Query, State},
};
use az_memory_model::{
    CaptureRequest, ImportRequest, ImportResult, NodeDraft, NodeKind, RevealedSecret, SourceList,
    SourceQuery, SourceView,
};
use regex::Regex;
use sqlx::Row;
use std::sync::Arc;

use super::nodes::SpaceQuery;

pub async fn capture(
    State(service): State<Arc<MemoryService>>,
    context: MemoryContext,
    Json(request): Json<CaptureRequest>,
) -> Result<Json<SourceView>> {
    validate_capture(&request)?;
    let mut transaction = service.pool.begin().await?;
    let space = require_space(
        &mut transaction,
        &context,
        request.space_id.as_deref(),
        true,
        false,
    )
    .await?;
    let existing = sqlx::query("SELECT id,ciphertext,status FROM plugin_memory_sources WHERE space_id=$1 AND created_by=$2 AND request_id=$3 FOR UPDATE")
        .bind(&space.id)
        .bind(&context.user_id)
        .bind(&request.request_id)
        .fetch_optional(&mut *transaction)
        .await?;
    if let Some(row) = existing {
        let status: String = row.try_get(2)?;
        if status == "deleted" {
            return Err(MemoryError::Stale);
        }
        let id: String = row.try_get(0)?;
        let ciphertext: Vec<u8> = row.try_get(1)?;
        let original = service
            .crypto
            .open(&format!("{}/source/{id}", space.id), &ciphertext)
            .await
            .map_err(MemoryError::storage)?;
        if original != request.text.as_bytes() {
            return Err(MemoryError::Input("同一请求 ID 不能提交不同内容".into()));
        }
        let source = source_view(&mut transaction, &context, &id).await?;
        transaction.commit().await?;
        return Ok(Json(source));
    }
    let isolated = isolate(&request.text, &[]);
    let title = if isolated.quarantined {
        "待整理的保密资料".to_owned()
    } else {
        source_title(&isolated.text)
    };
    let node = store::save_node(
        &mut transaction,
        &space.id,
        &context.user_id,
        NodeDraft {
            title,
            kind: NodeKind::Source,
            content: isolated.text.clone(),
            url: String::new(),
            tags: Vec::new(),
            version: None,
            aliases: Vec::new(),
        },
        None,
        "source",
        None,
    )
    .await?;
    let status = if isolated.quarantined {
        "quarantined"
    } else {
        "pending"
    };
    let ciphertext = service
        .crypto
        .seal(
            &format!("{}/source/{}", space.id, node.id),
            request.text.as_bytes(),
        )
        .await
        .map_err(MemoryError::storage)?;
    sqlx::query("INSERT INTO plugin_memory_sources(id,space_id,created_by,request_id,ciphertext,status,origin,reference,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)")
        .bind(&node.id)
        .bind(&space.id)
        .bind(&context.user_id)
        .bind(&request.request_id)
        .bind(ciphertext)
        .bind(status)
        .bind(&request.origin)
        .bind(&request.reference)
        .bind(access::now())
        .execute(&mut *transaction)
        .await?;
    for secret in &isolated.secrets {
        let ciphertext = service
            .crypto
            .seal(
                &format!("{}/secret/{}", space.id, secret.id),
                secret.value.as_bytes(),
            )
            .await
            .map_err(MemoryError::storage)?;
        sqlx::query("INSERT INTO plugin_memory_secrets(id,source_id,space_id,label,ciphertext,owner_id) VALUES($1,$2,$3,$4,$5,$6)")
            .bind(&secret.id)
            .bind(&node.id)
            .bind(&space.id)
            .bind(&secret.label)
            .bind(ciphertext)
            .bind(&context.user_id)
            .execute(&mut *transaction)
            .await?;
    }
    if !isolated.quarantined {
        sqlx::query("INSERT INTO plugin_memory_tasks(id,space_id,actor_id,state,available_at) VALUES($1,$2,$3,'pending',$4)")
            .bind(&node.id)
            .bind(&space.id)
            .bind(&context.user_id)
            .bind(access::now())
            .execute(&mut *transaction)
            .await?;
        for name in wiki_links(&isolated.text) {
            if name == node.title || name.starts_with("secret:") {
                continue;
            }
            let existing = sqlx::query("SELECT n.id,n.title,n.kind,n.content,n.url,n.tags,n.version,n.updated_at FROM plugin_memory_nodes n WHERE EXISTS(SELECT 1 FROM plugin_memory_ownership o WHERE o.node_id=n.id AND o.space_id=$1) AND n.title=$2 AND n.kind<>'SOURCE' LIMIT 1")
                .bind(&space.id)
                .bind(&name)
                .fetch_optional(&mut *transaction)
                .await?;
            let concept = if let Some(row) = existing {
                store::get_node(&mut transaction, &space.id, &row.try_get::<String, _>(0)?).await?
            } else {
                store::save_node(
                    &mut transaction,
                    &space.id,
                    &context.user_id,
                    NodeDraft {
                        title: name,
                        kind: NodeKind::Concept,
                        content: String::new(),
                        url: String::new(),
                        tags: Vec::new(),
                        version: None,
                        aliases: Vec::new(),
                    },
                    None,
                    "source",
                    None,
                )
                .await?
            };
            store::save_edge(
                &mut transaction,
                &space.id,
                az_memory_model::EdgeDraft {
                    source: node.id.clone(),
                    target: concept.id.clone(),
                    relation: "提及".into(),
                    evidence: "显式双向链接".into(),
                },
            )
            .await?;
            sqlx::query("INSERT INTO plugin_memory_evidence(node_id,source_id) VALUES($1,$2) ON CONFLICT DO NOTHING")
                .bind(concept.id)
                .bind(&node.id)
                .execute(&mut *transaction)
                .await?;
        }
    }
    let source = source_view(&mut transaction, &context, &node.id).await?;
    transaction.commit().await?;
    Ok(Json(source))
}

pub async fn list(
    State(service): State<Arc<MemoryService>>,
    Query(query): Query<SourceQuery>,
    context: MemoryContext,
) -> Result<Json<SourceList>> {
    if query.query.chars().count() > 256
        || query.offset < 0
        || !(1..=200).contains(&query.limit)
        || !matches!(
            query.status.as_str(),
            "" | "pending"
                | "processing"
                | "complete"
                | "conflict"
                | "failed"
                | "quarantined"
                | "recorded"
        )
    {
        return Err(MemoryError::Input("搜索或分页条件无效".into()));
    }
    let mut transaction = service.pool.begin().await?;
    let space = require_space(
        &mut transaction,
        &context,
        query.space_id.as_deref(),
        false,
        false,
    )
    .await?;
    let pattern = format!(
        "%{}%",
        query
            .query
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    );
    let condition = "s.space_id=$1 AND s.status<>'deleted' AND ($2='' OR s.status=$2) AND (n.title ILIKE $3 OR n.content ILIKE $3)";
    let total = sqlx::query_scalar::<_, i64>(&format!("SELECT count(*) FROM plugin_memory_sources s JOIN plugin_memory_nodes n ON n.id=s.id WHERE {condition}"))
        .bind(&space.id).bind(&query.status).bind(&pattern).fetch_one(&mut *transaction).await?;
    let ids = sqlx::query_scalar::<_, String>(&format!("SELECT s.id FROM plugin_memory_sources s JOIN plugin_memory_nodes n ON n.id=s.id WHERE {condition} ORDER BY n.updated_at DESC,s.id LIMIT $4 OFFSET $5"))
        .bind(&space.id)
        .bind(&query.status).bind(&pattern).bind(query.limit).bind(query.offset)
        .fetch_all(&mut *transaction)
        .await?;
    let truncated = query.offset.saturating_add(ids.len() as i64) < total;
    let mut sources = Vec::new();
    for id in ids {
        sources.push(source_view(&mut transaction, &context, &id).await?);
    }
    transaction.commit().await?;
    Ok(Json(SourceList {
        sources,
        truncated,
        total,
    }))
}

pub async fn get(
    State(service): State<Arc<MemoryService>>,
    Path(id): Path<String>,
    context: MemoryContext,
) -> Result<Json<SourceView>> {
    let mut transaction = service.pool.begin().await?;
    let source = source_view(&mut transaction, &context, &id).await?;
    transaction.commit().await?;
    Ok(Json(source))
}

pub async fn original(
    State(service): State<Arc<MemoryService>>,
    Path(id): Path<String>,
    context: MemoryContext,
) -> Result<Json<RevealedSecret>> {
    let mut transaction = service.pool.begin().await?;
    let source = source_view(&mut transaction, &context, &id).await?;
    if source.created_by != context.user_id || context.worker() {
        return Err(MemoryError::Access);
    }
    let ciphertext: Vec<u8> =
        sqlx::query_scalar("SELECT ciphertext FROM plugin_memory_sources WHERE id=$1")
            .bind(&id)
            .fetch_one(&mut *transaction)
            .await?;
    let plaintext = service
        .crypto
        .open(&format!("{}/source/{id}", source.space_id), &ciphertext)
        .await
        .map_err(MemoryError::storage)?;
    transaction.commit().await?;
    Ok(Json(RevealedSecret {
        value: String::from_utf8(plaintext).map_err(MemoryError::storage)?,
    }))
}

pub async fn retry(
    State(service): State<Arc<MemoryService>>,
    Path(id): Path<String>,
    context: MemoryContext,
) -> Result<Json<SourceView>> {
    let mut transaction = service.pool.begin().await?;
    let source = source_view(&mut transaction, &context, &id).await?;
    require_space(
        &mut transaction,
        &context,
        Some(&source.space_id),
        true,
        false,
    )
    .await?;
    if source.status == "quarantined" {
        return Err(MemoryError::Input(
            "请在对话中补充字段名称后重新提交资料".into(),
        ));
    }
    if !matches!(source.status.as_str(), "failed" | "conflict" | "pending") {
        return Err(MemoryError::Input("当前状态不能重试".into()));
    }
    sqlx::query("UPDATE plugin_memory_tasks SET state='pending',attempts=0,available_at=$2,lease=NULL,result=NULL,error=NULL WHERE id=$1")
        .bind(&id)
        .bind(access::now())
        .execute(&mut *transaction)
        .await?;
    sqlx::query("UPDATE plugin_memory_sources SET status='pending',error=NULL WHERE id=$1")
        .bind(&id)
        .execute(&mut *transaction)
        .await?;
    let source = source_view(&mut transaction, &context, &id).await?;
    transaction.commit().await?;
    Ok(Json(source))
}

pub async fn import(
    State(service): State<Arc<MemoryService>>,
    Query(query): Query<SpaceQuery>,
    context: MemoryContext,
    Json(import): Json<ImportRequest>,
) -> Result<(axum::http::StatusCode, Json<ImportResult>)> {
    let request = CaptureRequest {
        request_id: import.request_id,
        text: [import.title, import.url, import.text]
            .into_iter()
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        space_id: query.space_id,
        origin: "import".into(),
        reference: String::new(),
        clarifies: None,
    };
    let Json(source) = capture(State(service.clone()), context.clone(), Json(request)).await?;
    let mut transaction = service.pool.begin().await?;
    let node = store::get_node(&mut transaction, &source.space_id, &source.id).await?;
    let edges = store::links(
        &mut transaction,
        &source.space_id,
        &[source.id.clone()],
        false,
    )
    .await?;
    transaction.commit().await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(ImportResult {
            source: node,
            linked_nodes: edges.len() as i64,
        }),
    ))
}

pub(crate) async fn source_view(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    context: &MemoryContext,
    id: &str,
) -> Result<SourceView> {
    let space = require_node_space(transaction, context, id, false).await?;
    let row = sqlx::query("SELECT s.id,s.space_id,n.content,s.status,s.created_by,n.updated_at,s.error,n.title,n.version,s.origin FROM plugin_memory_sources s JOIN plugin_memory_nodes n ON n.id=s.id WHERE s.id=$1 AND s.status<>'deleted'")
        .bind(id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or(MemoryError::Missing)?;
    Ok(SourceView {
        id: row.try_get(0)?,
        space_id: row.try_get(1)?,
        text: row.try_get(2)?,
        status: row.try_get(3)?,
        created_by: row.try_get(4)?,
        updated_at: row.try_get(5)?,
        secrets: secrets::summaries_in(transaction, context, &space.id, Some(id)).await?,
        error: row.try_get(6)?,
        title: row.try_get(7)?,
        version: row.try_get(8)?,
        origin: row.try_get(9)?,
        can_edit: space.role.can_write()
            && !context.worker()
            && row.try_get::<String, _>(4)? == context.user_id,
        can_delete: space.role.can_write() && !context.worker(),
    })
}

pub(crate) fn source_title(text: &str) -> String {
    if let Ok(serde_json::Value::Object(object)) = serde_json::from_str::<serde_json::Value>(text) {
        for key in [
            "title", "project", "website", "name", "项目", "标题", "网站",
        ] {
            if let Some(value) = object.get(key).and_then(serde_json::Value::as_str) {
                if !value.is_empty() && !value.contains("[[secret:") {
                    return value.chars().take(100).collect();
                }
            }
        }
        return "账号与资料".into();
    }
    text.lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| {
            Regex::new(r"\[\[secret:[a-f0-9]{32}\]\]")
                .expect("固定引用正则有效")
                .replace_all(line, "[保密字段]")
                .chars()
                .take(100)
                .collect()
        })
        .filter(|value: &String| !value.is_empty())
        .unwrap_or_else(|| "新资料".into())
}

fn wiki_links(text: &str) -> Vec<String> {
    Regex::new(r"\[\[([^\[\]\n]{1,160})\]\]")
        .expect("固定 wiki 链接正则有效")
        .captures_iter(text)
        .filter_map(|capture| {
            capture.get(1).map(|value| {
                value
                    .as_str()
                    .split('|')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_owned()
            })
        })
        .filter(|value| !value.is_empty())
        .take(40)
        .collect()
}

fn validate_capture(request: &CaptureRequest) -> Result<()> {
    if request.text.trim().is_empty()
        || request.text.as_bytes().len() > 100_000
        || request.request_id.is_empty()
        || request.request_id.len() > 160
        || !matches!(request.origin.as_str(), "chat" | "note" | "import")
        || request.reference.len() > 256
    {
        return Err(MemoryError::Input("收件内容或来源无效".into()));
    }
    Ok(())
}
