use crate::{MemoryNode, NodeDraft};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureRequest {
    pub request_id: String,
    pub text: String,
    #[serde(default)]
    pub space_id: Option<String>,
    #[serde(default = "chat_origin")]
    pub origin: String,
    #[serde(default)]
    pub reference: String,
    #[serde(default)]
    pub clarifies: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretSummary {
    pub id: String,
    pub label: String,
    pub source_id: String,
    #[serde(default)]
    pub can_reveal: bool,
    #[serde(default)]
    pub can_manage: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceView {
    pub id: String,
    pub space_id: String,
    pub text: String,
    pub status: String,
    pub created_by: String,
    pub updated_at: i64,
    #[serde(default)]
    pub secrets: Vec<SecretSummary>,
    pub error: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub version: i64,
    #[serde(default)]
    pub origin: String,
    #[serde(default)]
    pub can_edit: bool,
    #[serde(default)]
    pub can_delete: bool,
}

/// 来源修订必须携带预览时的版本，原文仍由收件管线隔离和加密。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceUpdate {
    pub text: String,
    pub version: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceQuery {
    pub space_id: Option<String>,
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub offset: i64,
    #[serde(default = "source_limit")]
    pub limit: i64,
}

fn source_limit() -> i64 {
    200
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRequest {
    pub space_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompilationTask {
    pub id: String,
    pub lease: String,
    pub source: SourceView,
    pub existing: Vec<MemoryNode>,
    pub model_binding: Option<String>,
    pub instructions: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryProposal {
    pub draft: NodeDraft,
    #[serde(default)]
    pub existing_id: Option<String>,
    #[serde(default)]
    pub base_version: Option<i64>,
    #[serde(default)]
    pub secret_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationProposal {
    pub source_index: usize,
    pub target_index: usize,
    pub relation: String,
    #[serde(default)]
    pub evidence: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompilationResult {
    pub entries: Vec<EntryProposal>,
    #[serde(default)]
    pub relations: Vec<RelationProposal>,
    #[serde(default)]
    pub needs_review: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSubmission {
    pub lease: String,
    pub result: CompilationResult,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskFailure {
    pub lease: String,
    #[serde(default = "model_unavailable")]
    pub code: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewRequest {
    pub accept: bool,
    #[serde(default)]
    pub versions: std::collections::BTreeMap<String, i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SourceList {
    pub sources: Vec<SourceView>,
    pub truncated: bool,
    #[serde(default)]
    pub total: i64,
}

fn chat_origin() -> String {
    "chat".into()
}

fn model_unavailable() -> String {
    "model_unavailable".into()
}
