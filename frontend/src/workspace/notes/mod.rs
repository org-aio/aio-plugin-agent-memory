mod composer;
mod dialogs;
mod item;

use crate::{state::MemoryState, transport};
use az_memory_model::{SourceList, SourceView};
use az_ui_components::{
    admin::{EmptyState, RequestState},
    button::{Button, ButtonSize, ButtonVariant},
    input::Input,
};
use dioxus::prelude::*;
use dioxus_icons::lucide::{ArrowLeft, ArrowRight, Search};
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct Draft {
    pub text: String,
    pub request_id: String,
}

impl Default for Draft {
    fn default() -> Self {
        Self {
            text: String::new(),
            request_id: uuid::Uuid::new_v4().to_string(),
        }
    }
}

/// 草稿按空间保存，切换视图不丢输入；失败重试复用请求 ID。
#[derive(Clone, Default)]
pub struct NotesState {
    pub drafts: BTreeMap<String, Draft>,
    pub saving: bool,
    pub revision: u64,
}

#[derive(Clone)]
enum ActiveDialog {
    Detail(String),
    Edit(SourceView),
    Delete(SourceView),
}

#[component]
pub fn Notes() -> Element {
    let state = use_context::<Signal<MemoryState>>();
    let notes = use_context::<Signal<NotesState>>();
    let mut search_input = use_signal(String::new);
    let mut query = use_signal(String::new);
    let mut status = use_signal(String::new);
    let mut offset = use_signal(|| 0_i64);
    let mut dialog = use_signal(|| None::<ActiveDialog>);
    let space_id = state.read().space_id.clone();
    let revision = notes.read().revision;
    let search = query();
    let filter = status();
    let start = offset();
    let mut results = use_resource(use_reactive!(|(
        space_id,
        revision,
        search,
        filter,
        start,
    )| async move {
        let _ = revision;
        let Some(space_id) = space_id else {
            return Ok(SourceList {
                sources: vec![],
                total: 0,
                truncated: false,
            });
        };
        let encoded = js_sys::encode_uri_component(&search)
            .as_string()
            .unwrap_or_default();
        let path = format!("/sources?query={encoded}&status={filter}&offset={start}&limit=24");
        transport::space_request::<SourceList>(
            "GET",
            &path,
            Some(&space_id),
            serde_json::Value::Null,
        )
        .await
    }));
    let response = results.read().clone();
    let loading = !results.finished();
    rsx! {
        div { class: "memory-notes",
            composer::Composer { on_saved: move |_| offset.set(0) }
            section { class: "memory-feed", aria_label: "笔记列表",
                header { class: "memory-feed__toolbar",
                    h2 { "所有记录" }
                    form { class: "memory-search", onsubmit: move |event: FormEvent| {
                        event.prevent_default(); offset.set(0); query.set(search_input().trim().to_owned());
                    },
                        Input { aria_label: "搜索记录", placeholder: "搜索记录", value: search_input(), maxlength: "256",
                            oninput: move |event: FormEvent| search_input.set(event.value()) }
                        Button { r#type: "submit", size: ButtonSize::IconSm, variant: ButtonVariant::Ghost,
                            title: "搜索", aria_label: "搜索", Search { size: 16 } }
                    }
                    select { class: "memory-filter", aria_label: "整理状态", value: status(),
                        onchange: move |event| { offset.set(0); status.set(event.value()); },
                        option { value: "", "全部状态" }
                        option { value: "pending", "待整理" }
                        option { value: "processing", "整理中" }
                        option { value: "complete", "已整理" }
                        option { value: "failed", "整理失败" }
                        option { value: "conflict", "待核实" }
                        option { value: "quarantined", "保密暂存" }
                        option { value: "recorded", "对话记录" }
                    }
                }
                if loading { RequestState {} }
                else { match response {
                    Some(Ok(list)) => rsx! {
                        if list.sources.is_empty() {
                            EmptyState { title: if query().is_empty() && status().is_empty() { "暂无记录" } else { "没有匹配的记录" } }
                        }
                        div { class: "memory-feed__items",
                            for source in list.sources {
                                item::NoteItem { key: "{source.id}", source,
                                    on_open: move |id| dialog.set(Some(ActiveDialog::Detail(id))),
                                    on_edit: move |source| dialog.set(Some(ActiveDialog::Edit(source))),
                                    on_delete: move |source| dialog.set(Some(ActiveDialog::Delete(source))),
                                }
                            }
                        }
                        footer { class: "memory-pagination",
                            span { "{list.total} 条记录" }
                            Button { size: ButtonSize::IconSm, variant: ButtonVariant::Outline, title: "上一页", aria_label: "上一页",
                                disabled: offset() == 0, onclick: move |_| offset.set((offset() - 24).max(0)), ArrowLeft { size: 16 } }
                            span { "{offset() / 24 + 1} / {(list.total + 23).max(24) / 24}" }
                            Button { size: ButtonSize::IconSm, variant: ButtonVariant::Outline, title: "下一页", aria_label: "下一页",
                                disabled: !list.truncated, onclick: move |_| offset.set(offset() + 24), ArrowRight { size: 16 } }
                        }
                    },
                    Some(Err(error)) => rsx! { RequestState { error: Some(error), on_retry: move |_| results.restart() } },
                    None => rsx! { RequestState {} },
                } }
            }
        }
        if let Some(active) = dialog() {
            match active {
                ActiveDialog::Detail(id) => rsx! { dialogs::NoteDetail { key: "{id}", id, on_close: move |_| dialog.set(None), on_edit: move |source| dialog.set(Some(ActiveDialog::Edit(source))) } },
                ActiveDialog::Edit(source) => rsx! { dialogs::NoteEditor { key: "{source.id}", source, on_close: move |_| dialog.set(None) } },
                ActiveDialog::Delete(source) => rsx! { dialogs::NoteDelete { source, on_close: move |_| dialog.set(None), on_deleted: move |_| { offset.set(0); dialog.set(None); } } },
            }
        }
    }
}
