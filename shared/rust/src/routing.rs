use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteRequest {
    pub source_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryReference {
    pub id: String,
    pub title: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRoute {
    pub route: String,
    #[serde(default)]
    pub reply: Option<String>,
    #[serde(default)]
    pub context: String,
    #[serde(default)]
    pub citations: Vec<MemoryReference>,
    #[serde(default)]
    pub matched_node_ids: Vec<String>,
    #[serde(default)]
    pub activated_node_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivationRequest {
    #[serde(default)]
    pub node_ids: Vec<String>,
}
