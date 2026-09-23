use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SpaceRole {
    Owner,
    Editor,
    Reader,
}

impl SpaceRole {
    pub fn can_write(self) -> bool {
        matches!(self, Self::Owner | Self::Editor)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySpace {
    pub id: String,
    pub title: String,
    pub personal: bool,
    pub role: SpaceRole,
    pub model_binding: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceDraft {
    pub title: String,
    #[serde(default)]
    pub model_binding: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberDraft {
    pub user_id: String,
    pub role: SpaceRole,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceMember {
    pub user_id: String,
    pub role: SpaceRole,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretGrant {
    pub user_id: String,
    #[serde(default = "true_value")]
    pub reveal: bool,
    #[serde(default)]
    pub manage: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RevealedSecret {
    pub value: String,
}

fn true_value() -> bool {
    true
}
