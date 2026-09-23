use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NodeKind {
    Note,
    Concept,
    Person,
    Event,
    Source,
    Project,
}

impl NodeKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Note => "笔记",
            Self::Concept => "概念",
            Self::Person => "人物",
            Self::Event => "事件",
            Self::Source => "来源",
            Self::Project => "项目",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryNode {
    pub id: String,
    pub title: String,
    pub kind: NodeKind,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "initial_version")]
    pub version: i64,
    #[serde(default)]
    pub updated_at: i64,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeDraft {
    pub title: String,
    #[serde(default = "default_kind")]
    pub kind: NodeKind,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub version: Option<i64>,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub relation: String,
    #[serde(default)]
    pub evidence: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgeDraft {
    pub source: String,
    pub target: String,
    pub relation: String,
    #[serde(default)]
    pub evidence: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryGraph {
    pub nodes: Vec<MemoryNode>,
    pub edges: Vec<MemoryEdge>,
    pub total: i64,
    #[serde(default)]
    pub truncated: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequest {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub kind: Option<NodeKind>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecallRequest {
    pub query: String,
    #[serde(default = "default_recall_limit")]
    pub limit: usize,
    #[serde(default)]
    pub exclude_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisibilityRequest {
    pub node_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRequest {
    pub request_id: String,
    pub title: String,
    pub text: String,
    #[serde(default)]
    pub url: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub source: MemoryNode,
    pub linked_nodes: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextRequest {
    pub node_ids: Vec<String>,
    #[serde(default = "default_depth")]
    pub depth: usize,
    #[serde(default = "default_characters")]
    pub max_characters: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextResult {
    pub markdown: String,
    pub node_ids: Vec<String>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Failure {
    pub error: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WikiRevision {
    pub version: i64,
    pub draft: NodeDraft,
    pub author_id: String,
    pub author_type: String,
    pub source_id: Option<String>,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RollbackRequest {
    pub version: i64,
    pub current_version: i64,
}

fn initial_version() -> i64 {
    1
}

fn default_kind() -> NodeKind {
    NodeKind::Note
}

fn default_limit() -> usize {
    200
}

fn default_recall_limit() -> usize {
    8
}

fn default_depth() -> usize {
    1
}

fn default_characters() -> usize {
    16_000
}
