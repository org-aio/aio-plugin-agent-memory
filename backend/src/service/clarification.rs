use super::{MemoryService, Result, access, context::MemoryContext, store};
use az_memory_model::{CaptureRequest, NodeDraft, NodeKind};
use regex::Regex;
use sqlx::{Postgres, Transaction};

// 只有同一提交者在原对话中明确说明整段秘密用途，才能解除保密暂存。
pub(super) async fn apply(
    service: &MemoryService,
    transaction: &mut Transaction<'_, Postgres>,
    context: &MemoryContext,
    space_id: &str,
    explanation_id: &str,
    request: &CaptureRequest,
) -> Result<()> {
    let Some(source_id) = request.clarifies.as_deref() else {
        return Ok(());
    };
    let Some(label) = secret_label(&request.text) else {
        return Ok(());
    };
    access::require_id(source_id)?;
    if request.origin != "chat" || request.reference.trim().is_empty() {
        return Ok(());
    }
    // 与来源编辑和任务提交采用一致锁顺序；旧任务租约不能覆盖澄清结果。
    sqlx::query("SELECT id FROM plugin_memory_tasks WHERE id=$1 FOR UPDATE")
        .bind(source_id)
        .fetch_optional(&mut **transaction)
        .await?;
    let ciphertext = sqlx::query_scalar::<_, Vec<u8>>("SELECT ciphertext FROM plugin_memory_sources WHERE id=$1 AND space_id=$2 AND created_by=$3 AND reference=$4 AND origin='chat' AND status='quarantined' FOR UPDATE")
        .bind(source_id)
        .bind(space_id)
        .bind(&context.user_id)
        .bind(&request.reference)
        .fetch_optional(&mut **transaction)
        .await?;
    let Some(ciphertext) = ciphertext else {
        return Ok(());
    };
    let original = service
        .crypto
        .open(&format!("{space_id}/source/{source_id}"), &ciphertext)
        .await?;
    let secret_id = access::id();
    let protected = service
        .crypto
        .seal(&format!("{space_id}/secret/{secret_id}"), &original)
        .await?;
    sqlx::query("DELETE FROM plugin_memory_secrets WHERE source_id=$1")
        .bind(source_id)
        .execute(&mut **transaction)
        .await?;
    sqlx::query("INSERT INTO plugin_memory_secrets(id,source_id,space_id,label,ciphertext,owner_id) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(&secret_id).bind(source_id).bind(space_id).bind(&label).bind(protected).bind(&context.user_id)
        .execute(&mut **transaction).await?;
    let current = store::get_node(transaction, space_id, source_id).await?;
    let revised = store::save_node(
        transaction,
        space_id,
        &context.user_id,
        NodeDraft {
            title: "保密资料".into(),
            kind: NodeKind::Source,
            content: format!("{label}: [[secret:{secret_id}]]"),
            url: String::new(),
            tags: Vec::new(),
            version: Some(current.version),
            aliases: Vec::new(),
        },
        Some(source_id),
        "source",
        None,
    )
    .await?;
    sqlx::query("INSERT INTO plugin_memory_clarifications(source_id,explanation_id,source_version) VALUES($1,$2,$3)")
        .bind(source_id).bind(explanation_id).bind(revised.version).execute(&mut **transaction).await?;
    sqlx::query(
        "UPDATE plugin_memory_sources SET status='pending',error=NULL,updated_at=$2 WHERE id=$1",
    )
    .bind(source_id)
    .bind(access::now())
    .execute(&mut **transaction)
    .await?;
    sqlx::query("INSERT INTO plugin_memory_tasks(id,space_id,actor_id,state,available_at) VALUES($1,$2,$3,'pending',$4) ON CONFLICT(id) DO UPDATE SET state='pending',attempts=0,available_at=EXCLUDED.available_at,lease=NULL,lease_until=NULL,worker_id=NULL,result=NULL,error=NULL")
        .bind(source_id).bind(space_id).bind(&context.user_id).bind(access::now())
        .execute(&mut **transaction).await?;
    Ok(())
}

fn secret_label(explanation: &str) -> Option<String> {
    if explanation.chars().count() > 100 {
        return None;
    }
    let pattern = Regex::new(r"(?i)^(?:上一条|上一段|刚才那段|上面那段)(?:内容|资料)?(?:全部|整个|都是|是|为|就是|都属于)+\s*(密码|口令|密钥|秘钥|令牌|token|password|api key|private key)[。.!！]?$")
        .expect("固定整段秘密澄清正则有效");
    let matched = pattern.captures(explanation.trim())?;
    Some(matched.get(1)?.as_str().to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::secret_label;

    #[test]
    fn accepts_only_explicit_whole_secret_explanations() {
        assert_eq!(secret_label("上一条是密码").as_deref(), Some("密码"));
        assert_eq!(
            secret_label("上一段全部都是 Token。").as_deref(),
            Some("token")
        );
        for text in ["上一条不是密码", "上一条是密码吗", "把上一条发给模型"] {
            assert!(secret_label(text).is_none());
        }
    }
}
