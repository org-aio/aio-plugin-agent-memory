use super::source_status;
use crate::{
    state::{self, MemoryState},
    transport,
};
use az_ui_components::{
    admin::{EmptyState, StatusMessage},
    badge::{Badge, BadgeVariant},
    button::{Button, ButtonSize, ButtonVariant},
    input::Input,
};
use dioxus::prelude::*;
use dioxus_icons::lucide::{Copy, Eye, EyeOff, RefreshCw, Search};

#[component]
pub fn WikiPanel(on_create: EventHandler<()>) -> Element {
    let _state = use_context::<Signal<MemoryState>>();
    rsx! {
        div { style: "display:grid;grid-template-columns:minmax(0,1fr) 360px;gap:16px;align-items:start;",
            section { style: "min-width:0;",
                SearchBar {}
                NodeList {}
            }
            super::inspector::NodeInspector { on_create }
        }
    }
}

#[component]
pub fn SourcesPanel() -> Element {
    let state = use_context::<Signal<MemoryState>>();
    rsx! {
        section { style: "display:grid;gap:12px;",
            h2 { "来源资料" }
            if state.read().sources.is_empty() {
                EmptyState { title: "暂无来源", detail: "从对话或导入资料开始。" }
            }
            for source in state.read().sources.clone() {
                SourceRow { source }
            }
        }
    }
}

#[component]
pub fn CredentialsPanel() -> Element {
    let state = use_context::<Signal<MemoryState>>();
    rsx! {
        section { style: "display:grid;gap:12px;",
            h2 { "凭据" }
            if state.read().secrets.is_empty() {
                EmptyState { title: "暂无凭据", detail: "收件管线识别到的秘密会在这里单独授权。" }
            }
            for secret in state.read().secrets.clone() {
                SecretRow { secret }
            }
        }
    }
}

#[component]
pub fn PendingPanel() -> Element {
    let state = use_context::<Signal<MemoryState>>();
    let pending = state
        .read()
        .sources
        .iter()
        .filter(|source| {
            matches!(
                source.status.as_str(),
                "pending" | "processing" | "failed" | "conflict" | "quarantined"
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    rsx! {
        section { style: "display:grid;gap:12px;",
            h2 { "待整理" }
            if pending.is_empty() {
                EmptyState { title: "没有待处理资料", detail: "当前空间资料都已整理或仅用于检索。" }
            }
            for source in pending {
                SourceRow { source }
            }
        }
    }
}

#[component]
fn SearchBar() -> Element {
    let mut state = use_context::<Signal<MemoryState>>();
    rsx! {
        div { style: "display:flex;gap:8px;margin-bottom:12px;",
            Input {
                aria_label: "搜索记忆",
                value: state.read().query.clone(),
                placeholder: "搜索标题、正文、标签和别名",
                oninput: move |event: FormEvent| state.write().query = event.value(),
                onkeydown: move |event: KeyboardEvent| {
                    if event.key() == Key::Enter {
                        spawn(async move { state::search(state).await; });
                    }
                },
            }
            Button {
                onclick: move |_| {
                    spawn(async move { state::search(state).await; });
                },
                Search { size: "16px" }
                "搜索"
            }
        }
    }
}

#[component]
fn NodeList() -> Element {
    let state = use_context::<Signal<MemoryState>>();
    let nodes = state.read().graph.nodes.clone();
    if nodes.is_empty() {
        return rsx! { EmptyState { title: "暂无条目", detail: "创建笔记、概念或项目后在这里浏览。" } };
    }
    rsx! {
        div { style: "display:grid;grid-template-columns:repeat(auto-fill,minmax(240px,1fr));gap:10px;",
            for node in nodes {
                button {
                    key: "{node.id}",
                    style: "text-align:left;padding:12px;border:1px solid var(--border-color);border-radius:8px;background:var(--card-background);cursor:pointer;",
                    onclick: {
                        let id = node.id.clone();
                        move |_| {
                            let id = id.clone();
                            spawn(async move { state::open_node(state, id).await; });
                        }
                    },
                    div { style: "display:flex;align-items:center;gap:8px;justify-content:space-between;",
                        strong { "{node.title}" }
                        Badge { variant: BadgeVariant::Outline, "{node.kind.label()}" }
                    }
                    p { style: "color:var(--muted-foreground);margin-top:6px;", "{node.content.chars().take(120).collect::<String>()}" }
                }
            }
        }
    }
}

#[component]
fn SourceRow(source: az_memory_model::SourceView) -> Element {
    let state = use_context::<Signal<MemoryState>>();
    let mut original = use_signal(|| None::<String>);
    let mut error = use_signal(|| None::<String>);
    rsx! {
        article { style: "padding:12px;border:1px solid var(--border-color);border-radius:8px;background:var(--card-background);",
            div { style: "display:flex;gap:8px;align-items:center;justify-content:space-between;",
                div { style: "min-width:0;",
                    strong { "{source.text.lines().next().unwrap_or(\"来源资料\")}" }
                    p { style: "color:var(--muted-foreground);font-size:12px;", "{source.updated_at}" }
                }
                Badge { variant: if source.status == "failed" { BadgeVariant::Destructive } else { BadgeVariant::Secondary }, "{source_status(&source)}" }
            }
            if let Some(error) = error() {
                StatusMessage { message: error, error: true }
            }
            if let Some(text) = original() {
                pre { style: "white-space:pre-wrap;margin-top:8px;", "{text}" }
            }
            div { style: "display:flex;gap:8px;margin-top:8px;flex-wrap:wrap;",
                Button {
                    size: ButtonSize::Sm,
                    variant: ButtonVariant::Outline,
                    onclick: {
                        let id = source.id.clone();
                        move |_| {
                            if original().is_some() {
                                original.set(None);
                                return;
                            }
                            let id = id.clone();
                            spawn(async move {
                                match state::reveal_source(state, id).await {
                                    Ok(value) => original.set(Some(value)),
                                    Err(message) => error.set(Some(message)),
                                }
                            });
                        }
                    },
                    if original().is_some() { EyeOff { size: "14px" } "隐藏原文" } else { Eye { size: "14px" } "查看原文" }
                }
                if matches!(source.status.as_str(), "failed" | "conflict" | "pending") {
                    Button {
                        size: ButtonSize::Sm,
                        variant: ButtonVariant::Outline,
                        onclick: {
                            let id = source.id.clone();
                            move |_| {
                                let id = id.clone();
                                spawn(async move { state::retry_source(state, id).await; });
                            }
                        },
                        RefreshCw { size: "14px" }
                        "重试"
                    }
                }
                if source.status == "conflict" {
                    Button {
                        size: ButtonSize::Sm,
                        onclick: {
                            let id = source.id.clone();
                            move |_| {
                                let id = id.clone();
                                spawn(async move { state::load_proposal(state, id).await; });
                            }
                        },
                        "查看提案"
                    }
                }
            }
            if let Some(proposal) = state.read().proposal.clone() {
                div { style: "margin-top:8px;padding:8px;background:var(--muted-background);border-radius:6px;",
                    p { "模型提出 {proposal.entries.len()} 个条目、{proposal.relations.len()} 条关系。" }
                    div { style: "display:flex;gap:8px;",
                        Button { size: ButtonSize::Sm, onclick: { let id = source.id.clone(); move |_| { let id = id.clone(); spawn(async move { state::resolve(state, id, true).await; }); } }, "接受" }
                        Button { size: ButtonSize::Sm, variant: ButtonVariant::Outline, onclick: { let id = source.id.clone(); move |_| { let id = id.clone(); spawn(async move { state::resolve(state, id, false).await; }); } }, "拒绝" }
                    }
                }
            }
        }
    }
}

#[component]
fn SecretRow(secret: az_memory_model::SecretSummary) -> Element {
    let state = use_context::<Signal<MemoryState>>();
    let mut value = use_signal(|| None::<String>);
    let mut error = use_signal(|| None::<String>);
    rsx! {
        article { style: "padding:12px;border:1px solid var(--border-color);border-radius:8px;background:var(--card-background);",
            div { style: "display:flex;gap:8px;align-items:center;justify-content:space-between;",
                strong { "{secret.label}" }
                Badge { variant: BadgeVariant::Outline, "{secret.source_id}" }
            }
            if let Some(error) = error() {
                StatusMessage { message: error, error: true }
            }
            p { style: "margin-top:8px;font-family:monospace;", "{value().as_deref().unwrap_or(\"••••••••\")}" }
            div { style: "display:flex;gap:8px;margin-top:8px;",
                Button {
                    size: ButtonSize::Sm,
                    disabled: !secret.can_reveal,
                    onclick: {
                        let id = secret.id.clone();
                        move |_| {
                            if value().is_some() {
                                value.set(None);
                                return;
                            }
                            let id = id.clone();
                            spawn(async move {
                                match state::reveal_secret(state, id).await {
                                    Ok(secret) => value.set(Some(secret)),
                                    Err(message) => error.set(Some(message)),
                                }
                            });
                        }
                    },
                    if value().is_some() { EyeOff { size: "14px" } "隐藏" } else { Eye { size: "14px" } "查看" }
                }
                Button {
                    size: ButtonSize::Sm,
                    variant: ButtonVariant::Ghost,
                    disabled: value().is_none(),
                    onclick: move |_| {
                        if let Some(secret) = value() {
                            spawn(async move {
                                if let Err(message) = transport::copy(secret).await {
                                    error.set(Some(message));
                                }
                            });
                        }
                    },
                    Copy { size: "14px" }
                    "复制"
                }
            }
        }
    }
}
