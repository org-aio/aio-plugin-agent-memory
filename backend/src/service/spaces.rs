use super::{MemoryError, Result, context::MemoryContext};
use crate::service::MemoryService;
use axum::{
    Json,
    extract::{Path, State},
};
use az_memory_model::{MemberDraft, MemorySpace, SpaceDraft, SpaceMember, SpaceRole};
use sqlx::{Postgres, Row, Transaction};
use std::sync::Arc;

async fn load_space(
    transaction: &mut Transaction<'_, Postgres>,
    user: &str,
    id: Option<&str>,
    write: bool,
    owner: bool,
) -> Result<MemorySpace> {
    let id = match id {
        Some(id) => id.to_owned(),
        None => return Box::pin(load_personal_space(transaction, user)).await,
    };
    super::access::require_id(&id)?;
    let row = sqlx::query("SELECT s.id,s.title,s.personal_owner IS NOT NULL,m.role,s.model_binding FROM plugin_memory_spaces s JOIN plugin_memory_members m ON m.space_id=s.id WHERE s.id=$1 AND m.user_id=$2 FOR SHARE OF m")
        .bind(&id)
        .bind(user)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or(MemoryError::Access)?;
    let space = MemorySpace {
        id: row.try_get(0)?,
        title: row.try_get(1)?,
        personal: row.try_get(2)?,
        role: parse_role(row.try_get::<String, _>(3)?.as_str())?,
        model_binding: row.try_get(4)?,
    };
    if (write && !space.role.can_write()) || (owner && space.role != SpaceRole::Owner) {
        return Err(MemoryError::Access);
    }
    Ok(space)
}

async fn personal(transaction: &mut Transaction<'_, Postgres>, user: &str) -> Result<MemorySpace> {
    load_personal_space(transaction, user).await
}

async fn load_personal_space(
    transaction: &mut Transaction<'_, Postgres>,
    user: &str,
) -> Result<MemorySpace> {
    if let Some(row) = sqlx::query("SELECT id FROM plugin_memory_spaces WHERE personal_owner=$1")
        .bind(user)
        .fetch_optional(&mut **transaction)
        .await?
    {
        let id: String = row.try_get(0)?;
        return load_space(transaction, user, Some(&id), false, false).await;
    }
    let id = super::access::id();
    sqlx::query("INSERT INTO plugin_memory_spaces(id,title,personal_owner,created_by) VALUES($1,'个人记忆',$2,$2) ON CONFLICT(personal_owner) DO UPDATE SET personal_owner=EXCLUDED.personal_owner RETURNING id")
        .bind(&id)
        .bind(user)
        .fetch_one(&mut **transaction)
        .await?;
    sqlx::query("INSERT INTO plugin_memory_members(space_id,user_id,role) VALUES($1,$2,'OWNER') ON CONFLICT DO NOTHING")
        .bind(&id)
        .bind(user)
        .execute(&mut **transaction)
        .await?;
    load_space(transaction, user, Some(&id), false, false).await
}

pub(crate) async fn require_space(
    transaction: &mut Transaction<'_, Postgres>,
    context: &MemoryContext,
    id: Option<&str>,
    write: bool,
    owner: bool,
) -> Result<MemorySpace> {
    load_space(transaction, &context.user_id, id, write, owner).await
}

pub(crate) async fn require_node_space(
    transaction: &mut Transaction<'_, Postgres>,
    context: &MemoryContext,
    node_id: &str,
    write: bool,
) -> Result<MemorySpace> {
    super::access::require_id(node_id)?;
    let row = sqlx::query("SELECT space_id FROM plugin_memory_ownership WHERE node_id=$1")
        .bind(node_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or(MemoryError::Missing)?;
    let space_id: String = row.try_get(0)?;
    require_space(transaction, context, Some(&space_id), write, false).await
}

pub async fn list(
    State(service): State<Arc<MemoryService>>,
    context: MemoryContext,
) -> Result<Json<Vec<MemorySpace>>> {
    let mut transaction = service.pool.begin().await?;
    personal(&mut transaction, &context.user_id).await?;
    let rows = sqlx::query("SELECT s.id,s.title,s.personal_owner IS NOT NULL,m.role,s.model_binding FROM plugin_memory_spaces s JOIN plugin_memory_members m ON m.space_id=s.id WHERE m.user_id=$1 ORDER BY s.personal_owner IS NULL,s.title")
        .bind(&context.user_id)
        .fetch_all(&mut *transaction)
        .await?;
    transaction.commit().await?;
    rows.into_iter()
        .map(|row| {
            Ok(MemorySpace {
                id: row.try_get(0)?,
                title: row.try_get(1)?,
                personal: row.try_get(2)?,
                role: parse_role(row.try_get::<String, _>(3)?.as_str())?,
                model_binding: row.try_get(4)?,
            })
        })
        .collect::<Result<Vec<_>>>()
        .map(Json)
}

pub async fn create(
    State(service): State<Arc<MemoryService>>,
    context: MemoryContext,
    Json(draft): Json<SpaceDraft>,
) -> Result<Json<MemorySpace>> {
    validate(&draft)?;
    let mut transaction = service.pool.begin().await?;
    let id = super::access::id();
    sqlx::query(
        "INSERT INTO plugin_memory_spaces(id,title,created_by,model_binding) VALUES($1,$2,$3,$4)",
    )
    .bind(&id)
    .bind(draft.title.trim())
    .bind(&context.user_id)
    .bind(draft.model_binding.as_deref())
    .execute(&mut *transaction)
    .await?;
    sqlx::query("INSERT INTO plugin_memory_members(space_id,user_id,role) VALUES($1,$2,'OWNER')")
        .bind(&id)
        .bind(&context.user_id)
        .execute(&mut *transaction)
        .await?;
    let space = load_space(&mut transaction, &context.user_id, Some(&id), false, false).await?;
    transaction.commit().await?;
    Ok(Json(space))
}

pub async fn update(
    State(service): State<Arc<MemoryService>>,
    Path(id): Path<String>,
    context: MemoryContext,
    Json(draft): Json<SpaceDraft>,
) -> Result<Json<MemorySpace>> {
    validate(&draft)?;
    let mut transaction = service.pool.begin().await?;
    require_space(&mut transaction, &context, Some(&id), false, true).await?;
    sqlx::query("UPDATE plugin_memory_spaces SET title=$2,model_binding=$3 WHERE id=$1")
        .bind(&id)
        .bind(draft.title.trim())
        .bind(draft.model_binding.as_deref())
        .execute(&mut *transaction)
        .await?;
    let space = load_space(&mut transaction, &context.user_id, Some(&id), false, false).await?;
    transaction.commit().await?;
    Ok(Json(space))
}

pub async fn members(
    State(service): State<Arc<MemoryService>>,
    Path(id): Path<String>,
    context: MemoryContext,
) -> Result<Json<Vec<SpaceMember>>> {
    let mut transaction = service.pool.begin().await?;
    require_space(&mut transaction, &context, Some(&id), false, true).await?;
    let rows = sqlx::query(
        "SELECT user_id,role FROM plugin_memory_members WHERE space_id=$1 ORDER BY user_id",
    )
    .bind(&id)
    .fetch_all(&mut *transaction)
    .await?;
    transaction.commit().await?;
    rows.into_iter()
        .map(|row| {
            Ok(SpaceMember {
                user_id: row.try_get(0)?,
                role: parse_role(row.try_get::<String, _>(1)?.as_str())?,
            })
        })
        .collect::<Result<Vec<_>>>()
        .map(Json)
}

pub async fn add_member(
    State(service): State<Arc<MemoryService>>,
    Path(id): Path<String>,
    context: MemoryContext,
    Json(draft): Json<MemberDraft>,
) -> Result<Json<SpaceMember>> {
    let mut transaction = service.pool.begin().await?;
    let space = require_space(&mut transaction, &context, Some(&id), false, true).await?;
    validate_member(&space, &context, &draft.user_id)?;
    sqlx::query("INSERT INTO plugin_memory_members(space_id,user_id,role) VALUES($1,$2,$3) ON CONFLICT(space_id,user_id) DO UPDATE SET role=EXCLUDED.role")
        .bind(&id)
        .bind(&draft.user_id)
        .bind(role_name(draft.role))
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(Json(SpaceMember {
        user_id: draft.user_id,
        role: draft.role,
    }))
}

pub async fn remove_member(
    State(service): State<Arc<MemoryService>>,
    Path((id, user)): Path<(String, String)>,
    context: MemoryContext,
) -> Result<axum::http::StatusCode> {
    let mut transaction = service.pool.begin().await?;
    let space = require_space(&mut transaction, &context, Some(&id), false, true).await?;
    validate_member(&space, &context, &user)?;
    sqlx::query("DELETE FROM plugin_memory_members WHERE space_id=$1 AND user_id=$2")
        .bind(&id)
        .bind(&user)
        .execute(&mut *transaction)
        .await?;
    sqlx::query("DELETE FROM plugin_memory_secret_grants WHERE user_id=$2 AND secret_id IN (SELECT id FROM plugin_memory_secrets WHERE space_id=$1)")
        .bind(&id)
        .bind(&user)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub(crate) fn parse_role(value: &str) -> Result<SpaceRole> {
    match value {
        "OWNER" => Ok(SpaceRole::Owner),
        "EDITOR" => Ok(SpaceRole::Editor),
        "READER" => Ok(SpaceRole::Reader),
        _ => Err(MemoryError::Storage(anyhow::anyhow!("未知空间角色"))),
    }
}

pub(crate) fn role_name(role: SpaceRole) -> &'static str {
    match role {
        SpaceRole::Owner => "OWNER",
        SpaceRole::Editor => "EDITOR",
        SpaceRole::Reader => "READER",
    }
}

fn validate(draft: &SpaceDraft) -> Result<()> {
    let title = draft.title.trim();
    if title.is_empty()
        || title.chars().count() > 80
        || draft
            .model_binding
            .as_deref()
            .is_some_and(|value| value.len() > 160)
    {
        return Err(MemoryError::Input("空间配置无效".into()));
    }
    Ok(())
}

fn validate_member(space: &MemorySpace, context: &MemoryContext, target: &str) -> Result<()> {
    if space.personal || target.trim().is_empty() || target.len() > 128 || target == context.user_id
    {
        return Err(MemoryError::Input(
            "不能更改个人空间或自己的成员权限".into(),
        ));
    }
    Ok(())
}
