use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshHost {
    pub id: String,
    pub alias: String,
    pub hostname: String,
    pub user: String,
    pub port: u16,
    pub identity_file: String,
    pub device_id: String,
    pub device_label: String,
    pub status: String,
    pub last_error: Option<String>,
    pub version: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshHostDraft {
    pub alias: String,
    pub hostname: String,
    pub user: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_identity")]
    pub identity_file: String,
    pub device_id: String,
    #[serde(default)]
    pub version: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshDevice {
    pub id: String,
    pub label: String,
    pub platform: String,
    pub status: String,
    pub last_seen: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshApplyResult {
    pub host: SshHost,
    pub message: String,
    #[serde(default)]
    pub public_key: Option<String>,
}

fn default_port() -> u16 {
    22
}

fn default_identity() -> String {
    "id_ed25519".into()
}
