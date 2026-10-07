use crate::transport;
use az_memory_model::{
    CompilationResult, MemoryGraph, MemoryNode, MemorySpace, NodeDraft, NodeKind, SearchRequest,
    SecretSummary, SourceView, SshApplyResult, SshDevice, SshHost, SshHostDraft,
};
use dioxus::prelude::*;
use serde_json::json;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum View {
    Quick,
    Wiki,
    Graph,
    Sources,
    Credentials,
    Ssh,
    Pending,
}

impl View {
    pub fn label(self) -> &'static str {
        match self {
            Self::Quick => "随心记",
            Self::Wiki => "Wiki",
            Self::Graph => "图谱",
            Self::Sources => "来源",
            Self::Credentials => "凭据",
            Self::Ssh => "SSH",
            Self::Pending => "待整理",
        }
    }
}

#[derive(Clone)]
pub struct MemoryState {
    pub spaces: Vec<MemorySpace>,
    pub space_id: Option<String>,
    /// Wiki 列表与搜索使用的节点集合。
    pub graph: MemoryGraph,
    /// 关系图专用的知识节点概览，不受 Wiki 搜索影响。
    pub overview: MemoryGraph,
    pub sources: Vec<SourceView>,
    pub secrets: Vec<SecretSummary>,
    pub ssh_hosts: Vec<SshHost>,
    pub ssh_devices: Vec<SshDevice>,
    pub selected: Option<MemoryNode>,
    pub revisions: Vec<az_memory_model::WikiRevision>,
    pub proposal: Option<CompilationResult>,
    pub view: Option<View>,
    pub query: String,
    pub error: Option<String>,
    pub notice: Option<String>,
    pub busy: bool,
}

impl Default for MemoryState {
    fn default() -> Self {
        Self {
            spaces: Vec::new(),
            space_id: None,
            graph: MemoryGraph {
                nodes: Vec::new(),
                edges: Vec::new(),
                total: 0,
                truncated: false,
            },
            overview: MemoryGraph {
                nodes: Vec::new(),
                edges: Vec::new(),
                total: 0,
                truncated: false,
            },
            sources: Vec::new(),
            secrets: Vec::new(),
            ssh_hosts: Vec::new(),
            ssh_devices: Vec::new(),
            selected: None,
            revisions: Vec::new(),
            proposal: None,
            view: None,
            query: String::new(),
            error: None,
            notice: None,
            busy: false,
        }
    }
}

impl Default for View {
    fn default() -> Self {
        Self::Quick
    }
}

impl MemoryState {
    pub fn current_view(&self) -> View {
        self.view.unwrap_or_default()
    }
}

pub async fn load(mut state: Signal<MemoryState>) {
    state.write().busy = true;
    let result = async {
        let spaces: Vec<MemorySpace> = transport::request("GET", "/spaces", json!(null)).await?;
        let space_id = state
            .peek()
            .space_id
            .clone()
            .or_else(|| spaces.first().map(|space| space.id.clone()));
        let graph: MemoryGraph =
            transport::space_request("GET", "/graph", space_id.as_deref(), json!(null)).await?;
        let sources: az_memory_model::SourceList =
            transport::space_request("GET", "/sources", space_id.as_deref(), json!(null)).await?;
        let secrets: Vec<SecretSummary> =
            transport::space_request("GET", "/secrets", space_id.as_deref(), json!(null)).await?;
        let ssh_hosts: Vec<SshHost> = transport::request("GET", "/ssh/hosts", json!(null)).await?;
        let ssh_devices: Vec<SshDevice> = transport::request("GET", "/ssh/devices", json!(null))
            .await
            .unwrap_or_default();
        Ok::<_, String>((
            spaces,
            space_id,
            graph,
            sources.sources,
            secrets,
            ssh_hosts,
            ssh_devices,
        ))
    }
    .await;
    match result {
        Ok((spaces, space_id, graph, sources, secrets, ssh_hosts, ssh_devices)) => {
            let mut value = state.write();
            value.spaces = spaces;
            value.space_id = space_id;
            value.overview = graph.clone();
            value.graph = graph;
            value.sources = sources;
            value.secrets = secrets;
            value.ssh_hosts = ssh_hosts;
            value.ssh_devices = ssh_devices;
            value.error = None;
        }
        Err(error) => state.write().error = Some(error),
    }
    state.write().busy = false;
}

/// 只刷新关系图概览，不触碰 Wiki 列表和搜索结果。
pub async fn refresh_overview(mut state: Signal<MemoryState>) {
    let space_id = state.peek().space_id.clone();
    state.write().busy = true;
    let result =
        transport::space_request::<MemoryGraph>("GET", "/graph", space_id.as_deref(), json!(null))
            .await;
    match result {
        Ok(overview) => {
            let mut value = state.write();
            value.overview = overview;
            value.error = None;
        }
        Err(error) => state.write().error = Some(error),
    }
    state.write().busy = false;
}

pub async fn select_space(mut state: Signal<MemoryState>, id: String) {
    state.write().space_id = Some(id);
    refresh(state).await;
}

pub async fn refresh(mut state: Signal<MemoryState>) {
    let space_id = state.peek().space_id.clone();
    state.write().busy = true;
    let result = async {
        let graph: MemoryGraph =
            transport::space_request("GET", "/graph", space_id.as_deref(), json!(null)).await?;
        let sources: az_memory_model::SourceList =
            transport::space_request("GET", "/sources", space_id.as_deref(), json!(null)).await?;
        let secrets: Vec<SecretSummary> =
            transport::space_request("GET", "/secrets", space_id.as_deref(), json!(null)).await?;
        let ssh_hosts: Vec<SshHost> = transport::request("GET", "/ssh/hosts", json!(null)).await?;
        let ssh_devices: Vec<SshDevice> = transport::request("GET", "/ssh/devices", json!(null))
            .await
            .unwrap_or_default();
        Ok::<_, String>((graph, sources.sources, secrets, ssh_hosts, ssh_devices))
    }
    .await;
    match result {
        Ok((graph, sources, secrets, ssh_hosts, ssh_devices)) => {
            let mut value = state.write();
            value.overview = graph.clone();
            value.graph = graph;
            value.sources = sources;
            value.secrets = secrets;
            value.ssh_hosts = ssh_hosts;
            value.ssh_devices = ssh_devices;
            value.error = None;
        }
        Err(error) => state.write().error = Some(error),
    }
    state.write().busy = false;
}

pub async fn save_ssh_host(
    mut state: Signal<MemoryState>,
    id: Option<String>,
    draft: SshHostDraft,
) -> Result<SshHost, String> {
    let body = serde_json::to_value(draft).map_err(|error| error.to_string())?;
    let result = match id {
        Some(id) => transport::request::<SshHost>("PUT", &format!("/ssh/hosts/{id}"), body).await,
        None => transport::request::<SshHost>("POST", "/ssh/hosts", body).await,
    };
    match result {
        Ok(host) => {
            state.write().notice = Some("SSH 主机配置已保存".into());
            refresh(state).await;
            Ok(host)
        }
        Err(error) => {
            state.write().error = Some(error.clone());
            Err(error)
        }
    }
}

pub async fn delete_ssh_host(mut state: Signal<MemoryState>, id: String) -> Result<(), String> {
    match transport::request::<serde_json::Value>(
        "DELETE",
        &format!("/ssh/hosts/{id}"),
        json!(null),
    )
    .await
    {
        Ok(_) => {
            state.write().notice = Some("已删除 SSH 主机和设备配置".into());
            refresh(state).await;
            Ok(())
        }
        Err(error) => {
            state.write().error = Some(error.clone());
            Err(error)
        }
    }
}

pub async fn apply_ssh_host(
    state: Signal<MemoryState>,
    id: String,
) -> Result<SshApplyResult, String> {
    ssh_action(state, &id, "apply").await
}

pub async fn verify_ssh_host(
    state: Signal<MemoryState>,
    id: String,
) -> Result<SshApplyResult, String> {
    ssh_action(state, &id, "verify").await
}

async fn ssh_action(
    mut state: Signal<MemoryState>,
    id: &str,
    action: &str,
) -> Result<SshApplyResult, String> {
    match transport::request::<SshApplyResult>(
        "POST",
        &format!("/ssh/hosts/{id}/{action}"),
        json!(null),
    )
    .await
    {
        Ok(result) => {
            state.write().notice = Some(result.message.clone());
            refresh(state).await;
            Ok(result)
        }
        Err(error) => {
            state.write().error = Some(error.clone());
            Err(error)
        }
    }
}

pub async fn search(mut state: Signal<MemoryState>) {
    let space_id = state.peek().space_id.clone();
    let query = state.peek().query.clone();
    let request = SearchRequest {
        query,
        kind: None,
        limit: 200,
    };
    match transport::space_request(
        "POST",
        "/search",
        space_id.as_deref(),
        serde_json::to_value(request).unwrap_or(json!({})),
    )
    .await
    {
        Ok(graph) => {
            let mut value = state.write();
            value.graph = graph;
            value.error = None;
        }
        Err(error) => state.write().error = Some(error),
    }
}

pub async fn open_node(mut state: Signal<MemoryState>, id: String) {
    match transport::request::<MemoryNode>("GET", &format!("/nodes/{id}"), json!(null)).await {
        Ok(node) => {
            let revisions = transport::request::<Vec<az_memory_model::WikiRevision>>(
                "GET",
                &format!("/nodes/{id}/revisions"),
                json!(null),
            )
            .await
            .unwrap_or_default();
            let mut value = state.write();
            value.selected = Some(node);
            value.revisions = revisions;
            value.error = None;
        }
        Err(error) => state.write().error = Some(error),
    }
}

pub async fn save_node(
    mut state: Signal<MemoryState>,
    id: Option<String>,
    draft: NodeDraft,
) -> Result<(), String> {
    let space_id = state.peek().space_id.clone();
    let result = match id {
        Some(id) => {
            transport::request::<MemoryNode>(
                "PUT",
                &format!("/nodes/{id}"),
                serde_json::to_value(draft).map_err(|error| error.to_string())?,
            )
            .await
        }
        None => {
            transport::space_request::<MemoryNode>(
                "POST",
                "/nodes",
                space_id.as_deref(),
                serde_json::to_value(draft).map_err(|error| error.to_string())?,
            )
            .await
        }
    };
    match result {
        Ok(node) => {
            state.write().selected = Some(node);
            refresh(state).await;
            Ok(())
        }
        Err(error) => {
            state.write().error = Some(error.clone());
            Err(error)
        }
    }
}

pub async fn delete_node(mut state: Signal<MemoryState>, id: String) -> Result<(), String> {
    match transport::request::<serde_json::Value>("DELETE", &format!("/nodes/{id}"), json!(null))
        .await
    {
        Ok(_) => {
            state.write().selected = None;
            refresh(state).await;
            Ok(())
        }
        Err(error) => {
            state.write().error = Some(error.clone());
            Err(error)
        }
    }
}

pub async fn rollback(mut state: Signal<MemoryState>, id: String, version: i64, current: i64) {
    match transport::request::<MemoryNode>(
        "POST",
        &format!("/nodes/{id}/rollback"),
        json!({ "version": version, "currentVersion": current }),
    )
    .await
    {
        Ok(node) => {
            state.write().selected = Some(node);
            refresh(state).await;
        }
        Err(error) => state.write().error = Some(error),
    }
}

pub async fn reveal_secret(mut state: Signal<MemoryState>, id: String) -> Result<String, String> {
    match transport::request::<az_memory_model::RevealedSecret>(
        "POST",
        &format!("/secrets/{id}/reveal"),
        json!(null),
    )
    .await
    {
        Ok(secret) => Ok(secret.value),
        Err(error) => {
            state.write().error = Some(error.clone());
            Err(error)
        }
    }
}

pub async fn reveal_source(mut state: Signal<MemoryState>, id: String) -> Result<String, String> {
    match transport::request::<az_memory_model::RevealedSecret>(
        "POST",
        &format!("/sources/{id}/original"),
        json!(null),
    )
    .await
    {
        Ok(secret) => Ok(secret.value),
        Err(error) => {
            state.write().error = Some(error.clone());
            Err(error)
        }
    }
}

pub async fn retry_source(mut state: Signal<MemoryState>, id: String) {
    match transport::request::<SourceView>("POST", &format!("/sources/{id}/retry"), json!(null))
        .await
    {
        Ok(_) => refresh(state).await,
        Err(error) => state.write().error = Some(error),
    }
}

pub async fn load_proposal(mut state: Signal<MemoryState>, id: String) {
    match transport::request::<Option<CompilationResult>>(
        "GET",
        &format!("/sources/{id}/proposal"),
        json!(null),
    )
    .await
    {
        Ok(proposal) => state.write().proposal = proposal,
        Err(error) => state.write().error = Some(error),
    }
}

pub async fn resolve(mut state: Signal<MemoryState>, id: String, accept: bool) {
    match transport::request::<SourceView>(
        "POST",
        &format!("/sources/{id}/resolve"),
        json!({ "accept": accept, "versions": {} }),
    )
    .await
    {
        Ok(_) => {
            state.write().proposal = None;
            refresh(state).await;
        }
        Err(error) => state.write().error = Some(error),
    }
}

pub async fn capture(
    mut state: Signal<MemoryState>,
    title: String,
    text: String,
    url: String,
) -> Result<(), String> {
    let space_id = state.peek().space_id.clone();
    let request = az_memory_model::ImportRequest {
        request_id: uuid::Uuid::new_v4().to_string(),
        title,
        text,
        url,
    };
    match transport::space_request::<az_memory_model::ImportResult>(
        "POST",
        "/import",
        space_id.as_deref(),
        serde_json::to_value(request).unwrap_or(json!({})),
    )
    .await
    {
        Ok(_) => {
            state.write().notice = Some("已记下".into());
            refresh(state).await;
            Ok(())
        }
        Err(error) => {
            state.write().error = Some(error.clone());
            Err(error)
        }
    }
}

pub fn kind_options() -> Vec<(&'static str, NodeKind)> {
    vec![
        ("笔记", NodeKind::Note),
        ("概念", NodeKind::Concept),
        ("人物", NodeKind::Person),
        ("事件", NodeKind::Event),
        ("项目", NodeKind::Project),
    ]
}
