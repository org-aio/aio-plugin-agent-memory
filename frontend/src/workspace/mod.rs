mod dialogs;
mod graph_view;
mod inspector;
mod notes;
mod panels;

use crate::state::{self, MemoryState, View};
use az_ui_components::{
    admin::{RequestState, StatusMessage},
    button::{Button, ButtonSize, ButtonVariant},
    select::{Select, SelectItem},
};
use dioxus::prelude::*;
use dioxus_icons::lucide::{
    BookOpen, Database, KeyRound, ListChecks, Network, NotebookPen, Plus, RefreshCw,
};

#[component]
pub fn MemoryWorkspace() -> Element {
    let state = use_context::<Signal<MemoryState>>();
    let mut notes_state = use_signal(notes::NotesState::default);
    use_context_provider(|| notes_state);
    let mut node_dialog = use_signal(|| false);
    let mut source_dialog = use_signal(|| false);
    let space_options = state
        .read()
        .spaces
        .iter()
        .map(|space| SelectItem::new(space.id.clone(), space.title.clone()))
        .collect::<Vec<_>>();
    let selected_space = state.read().space_id.clone().unwrap_or_default();
    let can_write = state
        .read()
        .spaces
        .iter()
        .any(|space| space.id == selected_space && space.role.can_write());
    let current = state.read().current_view();
    rsx! {
        style { {include_str!("style.css")} }
        main { class: "memory-app",
            header { class: "memory-header",
                h1 { "记忆" }
                div { class: "memory-space",
                    Select { value: selected_space, options: space_options, aria_label: "知识空间",
                        disabled: state.read().busy || notes_state.read().saving,
                        on_value_change: move |id| { spawn(async move { state::select_space(state, id).await; }); }
                    }
                }
                div { class: "memory-header__actions",
                    Button {
                        variant: ButtonVariant::Outline,
                        size: ButtonSize::IconSm, title: "刷新", aria_label: "刷新",
                        disabled: state.read().busy,
                        onclick: move |_| {
                            notes_state.write().revision += 1;
                            spawn(async move { state::refresh(state).await; });
                        },
                        RefreshCw { size: "16px" }
                    }
                    if current != View::Quick { Button {
                        disabled: !can_write || state.read().busy,
                        onclick: move |_| node_dialog.set(true),
                        Plus { size: "16px" }
                        "新建条目"
                    } }
                    Button {
                        variant: ButtonVariant::Secondary,
                        disabled: !can_write || state.read().busy || notes_state.read().saving,
                        onclick: move |_| source_dialog.set(true),
                        "导入资料"
                    }
                }
            }
            nav { class: "memory-tabs", role: "tablist", aria_label: "记忆视图",
                ViewButton { view: View::Quick, current, icon: rsx! { NotebookPen { size: "16px" } } }
                ViewButton { view: View::Wiki, current, icon: rsx! { BookOpen { size: "16px" } } }
                ViewButton { view: View::Graph, current, icon: rsx! { Network { size: "16px" } } }
                ViewButton { view: View::Sources, current, icon: rsx! { Database { size: "16px" } } }
                ViewButton { view: View::Credentials, current, icon: rsx! { KeyRound { size: "16px" } } }
                ViewButton { view: View::Pending, current, icon: rsx! { ListChecks { size: "16px" } } }
            }
            if let Some(error) = state.read().error.clone() {
                StatusMessage { message: error, error: true }
            }
            if let Some(notice) = state.read().notice.clone() {
                StatusMessage { message: notice }
            }
                section { class: "memory-content", role: "tabpanel", aria_label: current.label(),
                    if state.read().spaces.is_empty() && state.read().busy {
                        RequestState {}
                    } else {
                        match current {
                            View::Quick => rsx! { notes::Notes { key: "{state.read().space_id.clone().unwrap_or_default()}" } },
                            View::Wiki => rsx! { panels::WikiPanel { on_create: move |_| node_dialog.set(true) } },
                            View::Graph => rsx! { graph_view::GraphPanel {} },
                            View::Sources => rsx! { panels::SourcesPanel {} },
                            View::Credentials => rsx! { panels::CredentialsPanel {} },
                            View::Pending => rsx! { panels::PendingPanel {} },
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
    let notes = use_context::<Signal<notes::NotesState>>();
    rsx! {
        Button {
            variant: if view == current { ButtonVariant::Secondary } else { ButtonVariant::Ghost },
            size: ButtonSize::Default,
            role: "tab", aria_selected: view == current, disabled: notes.read().saving,
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
        "complete" => "已整理",
        "conflict" => "待核实",
        "failed" => "整理失败",
        "quarantined" => "保密暂存",
        "recorded" => "对话记录",
        _ => "未知",
    }
}
