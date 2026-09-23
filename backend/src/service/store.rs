use super::{MemoryError, Result, access};
use az_memory_model::{
    EdgeDraft, MemoryEdge, MemoryGraph, MemoryNode, NodeDraft, NodeKind, SearchRequest,
    WikiRevision,
};
use sqlx::{Postgres, Row, Transaction};

const VISIBLE: &str = "EXISTS (SELECT 1 FROM plugin_memory_ownership o WHERE o.node_id=n.id AND o.space_id=$1) AND NOT EXISTS (SELECT 1 FROM plugin_memory_sources s WHERE s.id=n.id AND s.status='deleted') AND NOT EXISTS (SELECT 1 FROM plugin_memory_evidence e JOIN plugin_memory_sources s ON s.id=e.source_id WHERE e.node_id=n.id AND s.status='deleted')";

pub(crate) async fn get_node(
    transaction: &mut Transaction<'_, Postgres>,
    space_id: &str,
    id: &str,
) -> Result<MemoryNode> {
    access::require_id(id)?;
    let row = sqlx::query(&format!(
        "SELECT n.id,n.title,n.kind,n.content,n.url,n.tags,n.version,n.updated_at FROM plugin_memory_nodes n WHERE {VISIBLE} AND n.id=$2"
    ))
    .bind(space_id)
    .bind(id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(MemoryError::Missing)?;
    node(transaction, row).await
}

pub(crate) async fn save_node(
    transaction: &mut Transaction<'_, Postgres>,
    space_id: &str,
    user_id: &str,
    draft: NodeDraft,
    existing: Option<&str>,
    author: &str,
    source_id: Option<&str>,
) -> Result<MemoryNode> {
    let value = validate_draft(draft)?;
    let id = match existing {
        Some(id) => id.to_owned(),
        None => access::id(),
    };
    if let Some(existing) = existing {
        let current = get_node(transaction, space_id, existing).await?;
        if author != "source" && current.kind == NodeKind::Source {
            return Err(MemoryError::Input("来源只能通过收件管线新增或修订".into()));
        }
        let version = value
            .version
            .ok_or_else(|| MemoryError::Input("编辑时必须携带当前版本".into()))?;
        let changed = sqlx::query("UPDATE plugin_memory_nodes SET title=$2,kind=$3,content=$4,url=$5,tags=$6,updated_at=$7,version=version+1 WHERE id=$1 AND version=$8")
            .bind(&id)
            .bind(&value.title)
            .bind(kind_name(value.kind))
            .bind(&value.content)
            .bind(&value.url)
            .bind(serde_json::to_value(&value.tags)?)
            .bind(access::now())
            .bind(version)
            .execute(&mut **transaction)
            .await?;
        if changed.rows_affected() == 0 {
            return Err(MemoryError::Stale);
        }
        sqlx::query("UPDATE plugin_memory_ownership SET author_type=$2 WHERE node_id=$1")
            .bind(&id)
            .bind(author)
            .execute(&mut **transaction)
            .await?;
    } else {
        if author != "source" && value.kind == NodeKind::Source {
            return Err(MemoryError::Input("来源只能通过收件管线新增或修订".into()));
        }
        sqlx::query("INSERT INTO plugin_memory_nodes(id,title,kind,content,url,tags,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7)")
            .bind(&id)
            .bind(&value.title)
            .bind(kind_name(value.kind))
            .bind(&value.content)
            .bind(&value.url)
            .bind(serde_json::to_value(&value.tags)?)
            .bind(access::now())
            .execute(&mut **transaction)
            .await?;
        sqlx::query("INSERT INTO plugin_memory_ownership(node_id,space_id,created_by,author_type) VALUES($1,$2,$3,$4)")
            .bind(&id)
            .bind(space_id)
            .bind(user_id)
            .bind(author)
            .execute(&mut **transaction)
            .await?;
    }
    sqlx::query("DELETE FROM plugin_memory_aliases WHERE node_id=$1")
        .bind(&id)
        .execute(&mut **transaction)
        .await?;
    for alias in &value.aliases {
        sqlx::query("INSERT INTO plugin_memory_aliases(node_id,alias) VALUES($1,$2)")
            .bind(&id)
            .bind(alias)
            .execute(&mut **transaction)
            .await?;
    }
    let result = get_node(transaction, space_id, &id).await?;
    sqlx::query("INSERT INTO plugin_memory_revisions(id,node_id,version,draft,author_id,author_type,source_id,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
        .bind(access::id())
        .bind(&id)
        .bind(result.version)
        .bind(serde_json::to_value(&value)?)
        .bind(user_id)
        .bind(author)
        .bind(source_id)
        .bind(access::now())
        .execute(&mut **transaction)
        .await?;
    if let Some(source) = source_id {
        let source_version: i64 =
            sqlx::query_scalar("SELECT version FROM plugin_memory_nodes WHERE id=$1")
                .bind(source)
                .fetch_one(&mut **transaction)
                .await?;
        let revision_id: String = sqlx::query_scalar(
            "SELECT id FROM plugin_memory_revisions WHERE node_id=$1 AND version=$2",
        )
        .bind(&id)
        .bind(result.version)
        .fetch_one(&mut **transaction)
        .await?;
        sqlx::query("INSERT INTO plugin_memory_revision_sources(revision_id,source_id,source_version) VALUES($1,$2,$3)")
            .bind(revision_id)
            .bind(source)
            .bind(source_version)
            .execute(&mut **transaction)
            .await?;
    }
    Ok(result)
}

pub(crate) async fn save_edge(
    transaction: &mut Transaction<'_, Postgres>,
    space_id: &str,
    input: EdgeDraft,
) -> Result<MemoryEdge> {
    let value = validate_edge(input)?;
    get_node(transaction, space_id, &value.source).await?;
    get_node(transaction, space_id, &value.target).await?;
    let row = sqlx::query("INSERT INTO plugin_memory_edges(id,source,target,relation,evidence) VALUES($1,$2,$3,$4,$5) ON CONFLICT(source,target,relation) DO UPDATE SET evidence=EXCLUDED.evidence RETURNING id,source,target,relation,evidence")
        .bind(access::id())
        .bind(&value.source)
        .bind(&value.target)
        .bind(&value.relation)
        .bind(&value.evidence)
        .fetch_one(&mut **transaction)
        .await?;
    edge(row)
}

pub(crate) async fn visibility(
    transaction: &mut Transaction<'_, Postgres>,
    space_id: &str,
    ids: &[String],
) -> Result<Vec<String>> {
    if ids.len() > 400 {
        return Err(MemoryError::Input("引用数量超出限制".into()));
    }
    for id in ids {
        access::require_id(id)?;
    }
    let rows = sqlx::query(&format!(
        "SELECT n.id FROM plugin_memory_nodes n WHERE {VISIBLE} AND n.id = ANY($2)"
    ))
    .bind(space_id)
    .bind(ids)
    .fetch_all(&mut **transaction)
    .await?;
    let mut result = rows
        .into_iter()
        .map(|row| row.try_get(0))
        .collect::<std::result::Result<Vec<String>, _>>()?;
    result.sort();
    result.dedup();
    Ok(result)
}

pub(crate) async fn links(
    transaction: &mut Transaction<'_, Postgres>,
    space_id: &str,
    ids: &[String],
    internal_only: bool,
) -> Result<Vec<MemoryEdge>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    if ids.len() > 200 {
        return Err(MemoryError::Input("一次最多查询 200 个节点".into()));
    }
    let visible = visibility(transaction, space_id, ids).await?;
    if visible.len() != ids.len() {
        return Err(MemoryError::Missing);
    }
    let operator = if internal_only { "AND" } else { "OR" };
    let rows = sqlx::query(&format!(
        "SELECT e.id,e.source,e.target,e.relation,e.evidence FROM plugin_memory_edges e WHERE (e.source = ANY($2) {operator} e.target = ANY($2)) AND e.source IN (SELECT n.id FROM plugin_memory_nodes n WHERE {VISIBLE}) AND e.target IN (SELECT n.id FROM plugin_memory_nodes n WHERE {VISIBLE}) ORDER BY e.id LIMIT 801"
    ))
        .bind(space_id)
        .bind(ids)
        .fetch_all(&mut **transaction)
        .await?;
    rows.into_iter().map(edge).collect()
}

pub(crate) async fn graph(
    transaction: &mut Transaction<'_, Postgres>,
    space_id: &str,
    search: SearchRequest,
    include_edges: bool,
) -> Result<MemoryGraph> {
    if search.query.chars().count() > 256 || !(1..=200).contains(&search.limit) {
        return Err(MemoryError::Input("搜索条件超出限制".into()));
    }
    let pattern = format!(
        "%{}%",
        search
            .query
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    );
    let kind = search.kind.map(kind_name).unwrap_or("");
    let where_sql = format!(
        "{VISIBLE} AND NOT EXISTS (SELECT 1 FROM plugin_memory_sources s WHERE s.id=n.id AND s.status='recorded') AND (n.title ILIKE $2 OR n.content ILIKE $2 OR n.tags::text ILIKE $2 OR EXISTS (SELECT 1 FROM plugin_memory_aliases a WHERE a.node_id=n.id AND a.alias ILIKE $2)) AND ($3='' OR n.kind=$3)"
    );
    let total: i64 = sqlx::query_scalar(&format!(
        "SELECT count(*) FROM plugin_memory_nodes n WHERE {where_sql}"
    ))
    .bind(space_id)
    .bind(&pattern)
    .bind(kind)
    .fetch_one(&mut **transaction)
    .await?;
    let rows = sqlx::query(&format!(
        "SELECT n.id,n.title,n.kind,substring(n.content,1,180),n.url,n.tags,n.version,n.updated_at FROM plugin_memory_nodes n WHERE {where_sql} ORDER BY n.updated_at DESC,n.id LIMIT $4"
    ))
    .bind(space_id)
    .bind(&pattern)
    .bind(kind)
    .bind(search.limit as i64)
    .fetch_all(&mut **transaction)
    .await?;
    let mut nodes = Vec::new();
    for row in rows {
        nodes.push(node(transaction, row).await?);
    }
    let node_ids: Vec<String> = nodes.iter().map(|node| node.id.clone()).collect();
    let edges = if include_edges {
        links(transaction, space_id, &node_ids, true)
            .await?
            .into_iter()
            .take(800)
            .collect()
    } else {
        Vec::new()
    };
    Ok(MemoryGraph {
        truncated: total > nodes.len() as i64 || edges.len() >= 800,
        total,
        nodes,
        edges,
    })
}

pub(crate) async fn revisions(
    transaction: &mut Transaction<'_, Postgres>,
    space_id: &str,
    id: &str,
) -> Result<Vec<WikiRevision>> {
    get_node(transaction, space_id, id).await?;
    let rows = sqlx::query("SELECT version,draft,author_id,author_type,source_id,updated_at FROM plugin_memory_revisions WHERE node_id=$1 ORDER BY version DESC LIMIT 100")
        .bind(id)
        .fetch_all(&mut **transaction)
        .await?;
    rows.into_iter()
        .map(|row| {
            Ok(WikiRevision {
                version: row.try_get(0)?,
                draft: serde_json::from_value(row.try_get(1)?)?,
                author_id: row.try_get(2)?,
                author_type: row.try_get(3)?,
                source_id: row.try_get(4)?,
                updated_at: row.try_get(5)?,
            })
        })
        .collect()
}

async fn node(
    transaction: &mut Transaction<'_, Postgres>,
    row: sqlx::postgres::PgRow,
) -> Result<MemoryNode> {
    let id: String = row.try_get(0)?;
    let aliases =
        sqlx::query("SELECT alias FROM plugin_memory_aliases WHERE node_id=$1 ORDER BY alias")
            .bind(&id)
            .fetch_all(&mut **transaction)
            .await?
            .into_iter()
            .map(|row| row.try_get(0))
            .collect::<std::result::Result<Vec<String>, _>>()?;
    Ok(MemoryNode {
        id,
        title: row.try_get(1)?,
        kind: parse_kind(row.try_get::<String, _>(2)?.as_str())?,
        content: row.try_get(3)?,
        url: row.try_get(4)?,
        tags: serde_json::from_value(row.try_get(5)?)?,
        version: row.try_get(6)?,
        updated_at: row.try_get(7)?,
        aliases,
    })
}

fn edge(row: sqlx::postgres::PgRow) -> Result<MemoryEdge> {
    Ok(MemoryEdge {
        id: row.try_get(0)?,
        source: row.try_get(1)?,
        target: row.try_get(2)?,
        relation: row.try_get(3)?,
        evidence: row.try_get(4)?,
    })
}

pub(crate) fn validate_draft(draft: NodeDraft) -> Result<NodeDraft> {
    let title = draft.title.trim().to_owned();
    if title.is_empty() || title.chars().count() > 160 {
        return Err(MemoryError::Input("标题须为 1 至 160 个字符".into()));
    }
    if draft.content.chars().count() > 100_000 {
        return Err(MemoryError::Input("正文不能超过 100000 个字符".into()));
    }
    let url = draft.url.trim().to_owned();
    if url.len() > 2048
        || (!url.is_empty() && !url.starts_with("https://") && !url.starts_with("http://"))
    {
        return Err(MemoryError::Input("来源地址须为 HTTP 或 HTTPS URL".into()));
    }
    if draft.tags.len() > 12
        || draft
            .tags
            .iter()
            .any(|tag| !(1..=32).contains(&tag.trim().chars().count()))
    {
        return Err(MemoryError::Input(
            "最多 12 个标签，每个不超过 32 个字符".into(),
        ));
    }
    if draft.aliases.len() > 16
        || draft
            .aliases
            .iter()
            .any(|alias| !(1..=160).contains(&alias.trim().chars().count()))
    {
        return Err(MemoryError::Input(
            "最多 16 个别名，每个不超过 160 个字符".into(),
        ));
    }
    Ok(NodeDraft {
        title,
        url,
        tags: distinct(draft.tags),
        aliases: distinct(draft.aliases),
        ..draft
    })
}

fn validate_edge(draft: EdgeDraft) -> Result<EdgeDraft> {
    access::require_id(&draft.source)?;
    access::require_id(&draft.target)?;
    if draft.source == draft.target {
        return Err(MemoryError::Input("关系的两个节点不能相同".into()));
    }
    let relation = draft.relation.trim().to_owned();
    if !(1..=48).contains(&relation.chars().count()) {
        return Err(MemoryError::Input("关系名称须为 1 至 48 个字符".into()));
    }
    if draft.evidence.chars().count() > 2000 {
        return Err(MemoryError::Input("关系依据不能超过 2000 个字符".into()));
    }
    Ok(EdgeDraft { relation, ..draft })
}

fn distinct(values: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    values
        .into_iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

pub(crate) fn kind_name(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Note => "NOTE",
        NodeKind::Concept => "CONCEPT",
        NodeKind::Person => "PERSON",
        NodeKind::Event => "EVENT",
        NodeKind::Source => "SOURCE",
        NodeKind::Project => "PROJECT",
    }
}

pub(crate) fn parse_kind(value: &str) -> Result<NodeKind> {
    match value {
        "NOTE" => Ok(NodeKind::Note),
        "CONCEPT" => Ok(NodeKind::Concept),
        "PERSON" => Ok(NodeKind::Person),
        "EVENT" => Ok(NodeKind::Event),
        "SOURCE" => Ok(NodeKind::Source),
        "PROJECT" => Ok(NodeKind::Project),
        _ => Err(MemoryError::Storage(anyhow::anyhow!("未知节点类型"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_node_and_edge_drafts() {
        let node = validate_draft(NodeDraft {
            title: " 标题 ".into(),
            kind: NodeKind::Note,
            content: String::new(),
            url: String::new(),
            tags: vec![" tag ".into(), "tag".into()],
            version: None,
            aliases: vec![" alias ".into(), "alias".into()],
        })
        .expect("节点应有效");
        assert_eq!(node.title, "标题");
        assert_eq!(node.tags, vec!["tag"]);
        assert_eq!(node.aliases, vec!["alias"]);
        assert!(
            validate_edge(EdgeDraft {
                source: "a".repeat(32),
                target: "a".repeat(32),
                relation: "关系".into(),
                evidence: String::new(),
            })
            .is_err()
        );
    }
}
