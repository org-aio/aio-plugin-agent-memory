use super::{MemoryError, MemoryService, Result};
use sqlx::{Postgres, Row, Transaction};

pub(super) fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n")
        .trim_matches([' ', '\t', '\r', '\n'])
        .to_owned()
}

// 同一提交者的收件与编辑串行检查，避免不同请求 ID 同时写入重复内容。
pub(super) async fn lock(
    transaction: &mut Transaction<'_, Postgres>,
    space: &str,
    user: &str,
) -> Result<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1),hashtext($2))")
        .bind(space)
        .bind(user)
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

pub(super) async fn reject_duplicate(
    service: &MemoryService,
    transaction: &mut Transaction<'_, Postgres>,
    space: &str,
    user: &str,
    text: &str,
    except: Option<&str>,
) -> Result<()> {
    let isolated = super::isolate::isolate(text, &[]);
    let reference =
        regex::Regex::new(r"\[\[secret:[a-f0-9]{32}\]\]").expect("固定秘密引用正则有效");
    let template = normalize(&reference.replace_all(&isolated.text, "[protected]"));
    let rows = sqlx::query(
        "SELECT s.id,s.ciphertext FROM plugin_memory_sources s
         JOIN plugin_memory_nodes n ON n.id=s.id
         WHERE s.space_id=$1 AND s.created_by=$2 AND s.status<>'deleted'
         AND ($3::text IS NULL OR s.id<>$3)
         AND btrim(replace(regexp_replace(n.content, '\\[\\[secret:[a-f0-9]{32}\\]\\]', '[protected]', 'g'), E'\\r\\n', E'\\n'), E' \\t\\r\\n')=$4
         ORDER BY n.updated_at DESC,s.id FOR SHARE OF s",
    )
    .bind(space)
    .bind(user)
    .bind(except)
    .bind(template)
    .fetch_all(&mut **transaction)
    .await?;
    let normalized = normalize(text);
    // 净化文本只用于缩小候选范围；最终比较解密原文，不持久化秘密的散列。
    for row in rows {
        let id: String = row.try_get("id")?;
        let ciphertext: Vec<u8> = row.try_get("ciphertext")?;
        let original = service
            .crypto
            .open(&format!("{space}/source/{id}"), &ciphertext)
            .await?;
        if normalize(&String::from_utf8(original).map_err(MemoryError::storage)?) == normalized {
            return Err(MemoryError::Input("内容已存在，无需重复添加".into()));
        }
    }
    Ok(())
}
