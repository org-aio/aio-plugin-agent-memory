use super::{
    MemoryError, Result, access, intake, isolate::isolate, spaces::require_node_space, store,
};
use crate::{model::MemoryContext, service::MemoryService};
use axum::{
    Json,
    extract::{Path, State},
};
use az_memory_model::{NodeDraft, NodeKind, SourceUpdate, SourceView};
use sqlx::Row;
use std::sync::Arc;

/// 只有仍具备写权限的提交者能修订原文；任务租约与净化版本在同一事务替换。
pub async fn update(
    State(service): State<Arc<MemoryService>>,
    Path(id): Path<String>,
    context: MemoryContext,
    Json(request): Json<SourceUpdate>,
) -> Result<Json<SourceView>> {
    if context.worker() {
        return Err(MemoryError::Access);
    }
    if request.text.trim().is_empty() || request.text.len() > 100_000 || request.version < 1 {
        return Err(MemoryError::Input("正文不能为空且不能超过 100 KB".into()));
    }
    let mut transaction = service.pool.begin().await?;
    let space = require_node_space(&mut transaction, &context, &id, true).await?;
    // 与后台提交保持相同锁顺序，避免旧租约覆盖刚编辑的来源。
    sqlx::query("SELECT id FROM plugin_memory_tasks WHERE id=$1 FOR UPDATE")
        .bind(&id)
        .fetch_optional(&mut *transaction)
        .await?;
    sqlx::query("SELECT id FROM plugin_memory_sources WHERE id=$1 FOR UPDATE")
        .bind(&id)
        .fetch_optional(&mut *transaction)
        .await?;
    let source = intake::source_view(&mut transaction, &context, &id).await?;
    if !source.can_edit {
        return Err(MemoryError::Access);
    }
    if source.version != request.version {
        return Err(MemoryError::Stale);
    }

    let mut isolated = isolate(&request.text, &[]);
    let existing =
        sqlx::query("SELECT id,ciphertext FROM plugin_memory_secrets WHERE source_id=$1")
            .bind(&id)
            .fetch_all(&mut *transaction)
            .await?;
    let mut retained = Vec::new();
    // 值未变化的凭据保留 ID 和已有授权；新增或改值的凭据重新独立授权。
    for row in existing {
        let secret_id: String = row.try_get("id")?;
        let ciphertext: Vec<u8> = row.try_get("ciphertext")?;
        let value = service
            .crypto
            .open(&format!("{}/secret/{secret_id}", space.id), &ciphertext)
            .await?;
        if let Some(secret) = isolated
            .secrets
            .iter_mut()
            .find(|secret| secret.value.as_bytes() == value)
        {
            isolated.text = isolated.text.replace(&secret.id, &secret_id);
            secret.id = secret_id.clone();
            retained.push(secret_id);
        }
    }
    for secret in &isolated.secrets {
        if retained.contains(&secret.id) {
            continue;
        }
        let ciphertext = service
            .crypto
            .seal(
                &format!("{}/secret/{}", space.id, secret.id),
                secret.value.as_bytes(),
            )
            .await?;
        sqlx::query("INSERT INTO plugin_memory_secrets(id,source_id,space_id,label,ciphertext,owner_id) VALUES($1,$2,$3,$4,$5,$6)")
            .bind(&secret.id).bind(&id).bind(&space.id).bind(&secret.label).bind(ciphertext).bind(&context.user_id)
            .execute(&mut *transaction).await?;
        retained.push(secret.id.clone());
    }
    sqlx::query("DELETE FROM plugin_memory_secrets WHERE source_id=$1 AND NOT(id=ANY($2))")
        .bind(&id)
        .bind(&retained)
        .execute(&mut *transaction)
        .await?;
    let draft = NodeDraft {
        title: if isolated.quarantined {
            "待整理的保密资料".into()
        } else {
            intake::source_title(&isolated.text)
        },
        kind: NodeKind::Source,
        content: isolated.text,
        url: String::new(),
        tags: Vec::new(),
        aliases: Vec::new(),
        version: Some(request.version),
    };
    store::save_node(
        &mut transaction,
        &space.id,
        &context.user_id,
        draft,
        Some(&id),
        "source",
        None,
    )
    .await?;
    let ciphertext = service
        .crypto
        .seal(
            &format!("{}/source/{id}", space.id),
            request.text.as_bytes(),
        )
        .await?;
    let status = if isolated.quarantined {
        "quarantined"
    } else {
        "pending"
    };
    sqlx::query("UPDATE plugin_memory_sources SET ciphertext=$2,status=$3,error=NULL,updated_at=$4 WHERE id=$1")
        .bind(&id).bind(ciphertext).bind(status).bind(access::now()).execute(&mut *transaction).await?;
    let task_state = if isolated.quarantined {
        "cancelled"
    } else {
        "pending"
    };
    sqlx::query("INSERT INTO plugin_memory_tasks(id,space_id,actor_id,state,available_at) VALUES($1,$2,$3,$4,$5) ON CONFLICT(id) DO UPDATE SET state=EXCLUDED.state,attempts=0,available_at=EXCLUDED.available_at,lease=NULL,lease_until=NULL,worker_id=NULL,result=NULL,error=NULL")
        .bind(&id).bind(&space.id).bind(&context.user_id).bind(task_state).bind(access::now())
        .execute(&mut *transaction).await?;
    let source = intake::source_view(&mut transaction, &context, &id).await?;
    transaction.commit().await?;
    Ok(Json(source))
}
