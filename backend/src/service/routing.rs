use super::{
    MemoryError, Result,
    classifier::{self, ChatIntent},
    context::MemoryContext,
    graph,
    spaces::require_space,
    store,
};
use az_memory_model::{ChatRoute, ContextRequest, MemoryReference, RecallRequest, RouteRequest};

pub(crate) async fn route_in(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    context: &MemoryContext,
    space_id: &str,
    request: RouteRequest,
) -> Result<ChatRoute> {
    let source = super::intake::source_view(transaction, context, &request.source_id).await?;
    if source.space_id != space_id {
        return Err(MemoryError::Missing);
    }
    if source.status == "quarantined" {
        return Ok(ChatRoute {
            route: "quarantined".into(),
            reply: Some("已收下，资料已保密暂存。可以在这里补充这段资料的字段用途。".into()),
            context: String::new(),
            citations: Vec::new(),
            matched_node_ids: Vec::new(),
            activated_node_ids: Vec::new(),
        });
    }
    let decision = classifier::classify(&source.text);
    if decision.intent == ChatIntent::Greeting {
        return Ok(ChatRoute {
            route: "greeting".into(),
            reply: Some("你好！可以直接发资料让我记住，也可以问我之前记录的内容。".into()),
            context: String::new(),
            citations: Vec::new(),
            matched_node_ids: Vec::new(),
            activated_node_ids: Vec::new(),
        });
    }
    if decision.intent == ChatIntent::Save {
        let space = require_space(transaction, context, Some(space_id), false, false).await?;
        return Ok(ChatRoute {
            route: "save".into(),
            reply: Some(if space.model_binding.is_none() {
                "已收下，资料已保存。模型配置完成后继续整理。".into()
            } else {
                "已收下，正在后台整理。".into()
            }),
            context: String::new(),
            citations: Vec::new(),
            matched_node_ids: vec![source.id.clone()],
            activated_node_ids: vec![source.id],
        });
    }
    let recalled = graph::recall_in(
        transaction,
        space_id,
        RecallRequest {
            query: decision.query,
            limit: 4,
            exclude_ids: vec![source.id.clone()],
        },
    )
    .await?;
    let matched = recalled
        .nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<Vec<_>>();
    if decision.intent == ChatIntent::Recall {
        let mut included = matched.clone();
        for edge in store::links(transaction, space_id, &matched, false).await? {
            included.push(edge.source);
            included.push(edge.target);
        }
        included.sort();
        included.dedup();
        included.retain(|id| id != &source.id);
        included.truncate(24);
        let mut citations_out = Vec::new();
        for id in &included {
            let node = store::get_node(transaction, space_id, id).await?;
            citations_out.push(MemoryReference {
                id: node.id,
                title: node.title,
            });
        }
        let reply = if matched.is_empty() {
            "当前空间没有找到相关资料。".into()
        } else {
            let reference =
                regex::Regex::new(r"\[\[secret:[a-f0-9]{32}\]\]").expect("固定引用正则有效");
            let body = recalled
                .nodes
                .iter()
                .map(|node| {
                    format!(
                        "{}\n{}",
                        node.title,
                        reference.replace_all(&node.content, "[保密字段]")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n");
            format!("找到 {} 条相关资料：\n\n{}", matched.len(), body)
        };
        return Ok(ChatRoute {
            route: "recall".into(),
            reply: Some(reply),
            context: String::new(),
            citations: citations_out,
            matched_node_ids: matched,
            activated_node_ids: included,
        });
    }
    let context = if matched.is_empty() {
        None
    } else {
        Some(
            graph::context_in(
                transaction,
                space_id,
                ContextRequest {
                    node_ids: matched.clone(),
                    depth: 1,
                    max_characters: 6000,
                },
            )
            .await?,
        )
    };
    let included = context
        .as_ref()
        .map(|context| {
            context
                .node_ids
                .iter()
                .filter(|id| **id != source.id)
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut citations = Vec::new();
    for id in &included {
        let node = store::get_node(transaction, space_id, id).await?;
        citations.push(MemoryReference {
            id: node.id,
            title: node.title,
        });
    }
    Ok(ChatRoute {
        route: "model".into(),
        reply: None,
        context: context.map(|value| value.markdown).unwrap_or_default(),
        citations,
        matched_node_ids: matched
            .into_iter()
            .filter(|id| included.contains(id))
            .collect(),
        activated_node_ids: included,
    })
}
