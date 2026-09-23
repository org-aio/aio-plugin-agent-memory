use super::{MemoryError, Result, access, context::MemoryContext, spaces::require_space};
use crate::service::MemoryService;
use axum::{
    Json,
    extract::{Path, Query, State},
};
use az_memory_model::{RevealedSecret, SecretGrant, SecretSummary};
use sqlx::Row;
use std::sync::Arc;

use super::nodes::SpaceQuery;

pub async fn list(
    State(service): State<Arc<MemoryService>>,
    Query(query): Query<SpaceQuery>,
    context: MemoryContext,
) -> Result<Json<Vec<SecretSummary>>> {
    let mut transaction = service.pool.begin().await?;
    let space = require_space(
        &mut transaction,
        &context,
        query.space_id.as_deref(),
        false,
        false,
    )
    .await?;
    let summaries = summaries_in(&mut transaction, &context, &space.id, None).await?;
    transaction.commit().await?;
    Ok(Json(summaries))
}

pub async fn reveal(
    State(service): State<Arc<MemoryService>>,
    Path(id): Path<String>,
    context: MemoryContext,
) -> Result<Json<RevealedSecret>> {
    access::require_id(&id)?;
    let mut transaction = service.pool.begin().await?;
    let row = sqlx::query("SELECT s.space_id,s.ciphertext,s.owner_id,s.source_id,(s.owner_id=$2 OR coalesce(g.can_reveal,false)),(s.owner_id=$2 OR coalesce(g.can_manage,false)) FROM plugin_memory_secrets s LEFT JOIN plugin_memory_secret_grants g ON g.secret_id=s.id AND g.user_id=$2 WHERE s.id=$1")
        .bind(&id)
        .bind(&context.user_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(MemoryError::Missing)?;
    let space_id: String = row.try_get(0)?;
    require_space(&mut transaction, &context, Some(&space_id), false, false).await?;
    let can_reveal: bool = row.try_get(4)?;
    if context.worker() || !can_reveal {
        return Err(MemoryError::Access);
    }
    let ciphertext: Vec<u8> = row.try_get(1)?;
    let plaintext = service
        .crypto
        .open(&format!("{space_id}/secret/{id}"), &ciphertext)
        .await
        .map_err(MemoryError::storage)?;
    transaction.commit().await?;
    Ok(Json(RevealedSecret {
        value: String::from_utf8(plaintext).map_err(MemoryError::storage)?,
    }))
}

pub async fn grant(
    State(service): State<Arc<MemoryService>>,
    Path(id): Path<String>,
    context: MemoryContext,
    Json(grant): Json<SecretGrant>,
) -> Result<axum::http::StatusCode> {
    access::require_id(&id)?;
    let mut transaction = service.pool.begin().await?;
    let row = sqlx::query("SELECT s.space_id,s.ciphertext,s.owner_id,s.source_id,(s.owner_id=$2 OR coalesce(g.can_reveal,false)),(s.owner_id=$2 OR coalesce(g.can_manage,false)) FROM plugin_memory_secrets s LEFT JOIN plugin_memory_secret_grants g ON g.secret_id=s.id AND g.user_id=$2 WHERE s.id=$1")
        .bind(&id)
        .bind(&context.user_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(MemoryError::Missing)?;
    let space_id: String = row.try_get(0)?;
    require_space(&mut transaction, &context, Some(&space_id), false, false).await?;
    let can_manage: bool = row.try_get(5)?;
    if context.worker() || !can_manage {
        return Err(MemoryError::Access);
    }
    if grant.user_id.trim().is_empty() || grant.user_id.len() > 128 {
        return Err(MemoryError::Input("成员无效".into()));
    }
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM plugin_memory_members WHERE space_id=$1 AND user_id=$2 FOR SHARE)")
        .bind(&space_id)
        .bind(&grant.user_id)
        .fetch_one(&mut *transaction)
        .await?;
    if !exists {
        return Err(MemoryError::Input("目标用户不是当前空间成员".into()));
    }
    sqlx::query("INSERT INTO plugin_memory_secret_grants(secret_id,user_id,can_reveal,can_manage) VALUES($1,$2,$3,$4) ON CONFLICT(secret_id,user_id) DO UPDATE SET can_reveal=EXCLUDED.can_reveal,can_manage=EXCLUDED.can_manage")
        .bind(&id)
        .bind(&grant.user_id)
        .bind(grant.reveal)
        .bind(grant.manage)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub(crate) async fn summaries_in(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    context: &MemoryContext,
    space_id: &str,
    source_id: Option<&str>,
) -> Result<Vec<SecretSummary>> {
    let rows = sqlx::query("SELECT s.id,s.label,s.source_id,(s.owner_id=$2 OR coalesce(g.can_reveal,false)),(s.owner_id=$2 OR coalesce(g.can_manage,false)) FROM plugin_memory_secrets s LEFT JOIN plugin_memory_secret_grants g ON g.secret_id=s.id AND g.user_id=$2 WHERE s.space_id=$1 AND ($3='' OR s.source_id=$3) ORDER BY s.id LIMIT 200")
        .bind(space_id)
        .bind(&context.user_id)
        .bind(source_id.unwrap_or(""))
        .fetch_all(&mut **transaction)
        .await?;
    rows.into_iter()
        .map(|row| {
            Ok(SecretSummary {
                id: row.try_get(0)?,
                label: row.try_get(1)?,
                source_id: row.try_get(2)?,
                can_reveal: !context.worker() && row.try_get(3)?,
                can_manage: !context.worker() && row.try_get(4)?,
            })
        })
        .collect()
}
