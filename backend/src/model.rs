use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryContext {
    pub tenant_id: String,
    pub user_id: String,
    pub context_id: Option<String>,
}

impl MemoryContext {
    pub fn worker(&self) -> bool {
        self.context_id
            .as_deref()
            .is_some_and(|id| id.starts_with("service:"))
    }
}
