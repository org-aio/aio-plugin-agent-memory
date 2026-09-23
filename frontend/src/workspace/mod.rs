mod dialogs;
mod graph_view;
mod inspector;
mod panels;

use crate::state::{self, MemoryState, View};
use az_ui_components::{
    admin::{PageHeader, PageSurface, RequestState, StatusMessage},
    button::{Button, ButtonSize, ButtonVariant},
    select::{Select, SelectItem},
};
use dioxus::prelude::*;
use dioxus_icons::lucide::{BookOpen, Database, KeyRound, ListChecks, Network, Plus, RefreshCw};

#[component]
pub fn MemoryWorkspace() -> Element {
    let state = use_context::<Signal<MemoryState>>();
    let mut node_dialog = use_signal(|| false);
    let mut source_dialog = use_signal(|| false);
    let space_options = state
        .read()
        .spaces
        .iter()
        .map(|space| SelectItem::new(space.id.clone(), space.title.clone()))
        .collect::<Vec<_>>();
    let selected_space = state.read().space_id.clone().unwrap_or_default();
    let current = state.read().current_view();
    rsx! {
        PageSurface {
            PageHeader {
                title: "智能体记忆",
                detail: "管理知识空间、资料收件、图谱与凭据",
                div { class: "flex gap-2",
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: state.read().busy,
                        onclick: move |_| {
                            spawn(async move { state::refresh(state).await; });
                        },
                        RefreshCw { size: "16px" }
                        "刷新"
                    }
                    Button {
                        onclick: move |_| node_dialog.set(true),
                        Plus { size: "16px" }
                        "新建条目"
                    }
                    Button {
                        variant: ButtonVariant::Secondary,
                        onclick: move |_| source_dialog.set(true),
                        "导入资料"
                    }
                }
            }
            if let Some(error) = state.read().error.clone() {
                StatusMessage { message: error, error: true }
            }
            if let Some(notice) = state.read().notice.clone() {
                StatusMessage { message: notice }
            }
            div {
                style: "display:grid;grid-template-columns:260px minmax(0,1fr);gap:16px;align-items:start;",
                aside {
                    style: "display:flex;flex-direction:column;gap:12px;position:sticky;top:12px;",
                    section {
                        style: "padding:12px;border:1px solid var(--border-color);border-radius:8px;background:var(--card-background);",
                        label { style: "font-size:12px;color:var(--muted-foreground);", "知识空间" }
                        Select {
                            value: selected_space,
                            options: space_options,
                            aria_label: "知识空间",
                            on_value_change: move |id| {
                                spawn(async move { state::select_space(state, id).await; });
                            },
                        }
                    }
                    nav {
                        style: "display:grid;gap:4px;",
                        ViewButton { view: View::Wiki, current, icon: rsx! { BookOpen { size: "16px" } } }
                        ViewButton { view: View::Graph, current, icon: rsx! { Network { size: "16px" } } }
                        ViewButton { view: View::Sources, current, icon: rsx! { Database { size: "16px" } } }
                        ViewButton { view: View::Credentials, current, icon: rsx! { KeyRound { size: "16px" } } }
                        ViewButton { view: View::Pending, current, icon: rsx! { ListChecks { size: "16px" } } }
                    }
                }
                section {
                    style: "min-width:0;",
                    if state.read().spaces.is_empty() && state.read().busy {
                        RequestState {}
                    } else {
                        match current {
                            View::Wiki => rsx! { panels::WikiPanel { on_create: move |_| node_dialog.set(true) } },
                            View::Graph => rsx! { graph_view::GraphPanel {} },
                            View::Sources => rsx! { panels::SourcesPanel {} },
                            View::Credentials => rsx! { panels::CredentialsPanel {} },
                            View::Pending => rsx! { panels::PendingPanel {} },
                        }
                    }
                }
            }
        }
        if node_dialog() {
            dialogs::NodeDialog { on_close: move |_| node_dialog.set(false) }
        }
        if source_dialog() {
            dialogs::SourceDialog { on_close: move |_| source_dialog.set(false) }
        }
    }
}

#[component]
fn ViewButton(view: View, current: View, icon: Element) -> Element {
    let mut state = use_context::<Signal<MemoryState>>();
    rsx! {
        Button {
            variant: if view == current { ButtonVariant::Secondary } else { ButtonVariant::Ghost },
            size: ButtonSize::Default,
            style: "justify-content:flex-start;width:100%;",
            onclick: move |_| {
                state.write().view = Some(view);
            },
            {icon}
            "{view.label()}"
        }
    }
}

pub fn source_status(source: &az_memory_model::SourceView) -> &'static str {
    match source.status.as_str() {
        "pending" => "待整理",
        "processing" => "整理中",
        "complete" => "已完成",
        "conflict" => "待核实",
        "failed" => "失败",
        "quarantined" => "保密暂存",
        "recorded" => "对话记录",
        _ => "未知",
    }
}
