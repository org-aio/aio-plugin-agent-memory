use super::{NotesState, item::time_label};
use crate::{
    state::{self, MemoryState},
    transport,
};
use az_memory_model::{RevealedSecret, SourceView};
use az_ui_components::{
    admin::{RequestState, StatusMessage},
    button::{Button, ButtonSize, ButtonVariant},
    dialog::{Dialog, DialogDescription, DialogTitle},
    markdown::Markdown,
    textarea::Textarea,
};
use dioxus::prelude::*;
use dioxus_icons::lucide::{Pencil, Trash2, X};
use serde_json::{Value, json};

#[component]
pub(super) fn NoteDetail(
    id: String,
    on_close: EventHandler<()>,
    on_edit: EventHandler<SourceView>,
) -> Element {
    let mut result = use_resource(move || {
        let id = id.clone();
        async move {
            transport::request::<SourceView>("GET", &format!("/sources/{id}"), Value::Null).await
        }
    });
    let response = result.read().clone();
    rsx! {
        Dialog { class: "memory-dialog", open: true, on_open_change: move |open: bool| if !open { on_close.call(()); },
            header { class: "memory-dialog__heading", DialogTitle { "记录详情" }
                Button { size: ButtonSize::IconSm, variant: ButtonVariant::Ghost, title: "关闭", aria_label: "关闭详情", onclick: move |_| on_close.call(()), X { size: 18 } }
            }
            DialogDescription { "完整记录" }
            match response {
                Some(Ok(source)) => rsx! {
                    div { class: "memory-dialog__body", Markdown { source: source.text.clone(), link_base: "/", image_base: "/" } }
                    footer { class: "memory-dialog__footer",
                        time { "{time_label(source.updated_at)}" }
                        if source.can_edit { Button { onclick: move |_| on_edit.call(source.clone()), Pencil { size: 16 } "编辑记录" } }
                    }
                },
                Some(Err(error)) => rsx! { RequestState { error: Some(error), on_retry: move |_| result.restart() } },
                None => rsx! { RequestState {} },
            }
        }
    }
}

#[component]
pub(super) fn NoteEditor(source: SourceView, on_close: EventHandler<()>) -> Element {
    let state = use_context::<Signal<MemoryState>>();
    let mut notes = use_context::<Signal<NotesState>>();
    let mut text = use_signal(String::new);
    let mut version = use_signal(|| 0_i64);
    let mut loading = use_signal(|| true);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut preview = use_signal(|| false);
    let id = source.id.clone();
    use_future(move || {
        let id = id.clone();
        async move {
            let result = async {
                let current: SourceView =
                    transport::request("GET", &format!("/sources/{id}"), Value::Null).await?;
                let original: RevealedSecret =
                    transport::request("POST", &format!("/sources/{id}/original"), Value::Null)
                        .await?;
                Ok::<_, String>((current.version, original.value))
            }
            .await;
            match result {
                Ok((current, original)) => {
                    version.set(current);
                    text.set(original);
                }
                Err(message) => error.set(Some(message)),
            }
            loading.set(false);
        }
    });
    rsx! {
        Dialog { class: "memory-dialog", open: true, on_open_change: move |open: bool| if !open && !busy() { on_close.call(()); },
            header { class: "memory-dialog__heading", DialogTitle { "编辑记录" }
                Button { size: ButtonSize::IconSm, variant: ButtonVariant::Ghost, aria_label: "关闭编辑", title: "关闭", disabled: busy(), onclick: move |_| on_close.call(()), X { size: 18 } }
            }
            DialogDescription { "保存后重新整理此记录。" }
            if loading() { RequestState {} }
            form { class: "memory-edit-form", onsubmit: move |event: FormEvent| {
                event.prevent_default();
                if busy() || loading() || version() < 1 { return; }
                if text().trim().is_empty() || text().len() > 100_000 { error.set(Some("正文不能为空且不能超过 100 KB".into())); return; }
                busy.set(true); error.set(None);
                let id = source.id.clone(); let body = json!({"text": text(), "version": version()});
                spawn(async move {
                    match transport::request::<SourceView>("PUT", &format!("/sources/{id}"), body).await {
                        Ok(_) => {
                            notes.write().revision += 1;
                            state::refresh(state).await;
                            busy.set(false);
                            on_close.call(());
                            return;
                        },
                        Err(message) => error.set(Some(message)),
                    }
                    busy.set(false);
                });
            },
                if preview() {
                    div { class: "memory-dialog__body", Markdown { source: text(), link_base: "/", image_base: "/" } }
                } else {
                    Textarea { class: "memory-editor", aria_label: "编辑记录正文", rows: "14", value: text(), disabled: loading() || busy(),
                        oninput: move |event: FormEvent| text.set(event.value()) }
                }
                if let Some(message) = error() { StatusMessage { message, error: true } }
                footer { class: "memory-dialog__footer",
                    Button { r#type: "button", variant: ButtonVariant::Ghost, aria_pressed: preview(), onclick: move |_| preview.toggle(), if preview() { "继续编辑" } else { "预览" } }
                    Button { r#type: "button", variant: ButtonVariant::Outline, disabled: busy(), onclick: move |_| on_close.call(()), "取消" }
                    Button { r#type: "submit", disabled: busy() || loading() || version() < 1, if busy() { "保存中" } else { "保存修改" } }
                }
            }
        }
    }
}

#[component]
pub(super) fn NoteDelete(
    source: SourceView,
    on_close: EventHandler<()>,
    on_deleted: EventHandler<()>,
) -> Element {
    let mut state = use_context::<Signal<MemoryState>>();
    let mut notes = use_context::<Signal<NotesState>>();
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    rsx! {
        Dialog { class: "memory-confirm", open: true, on_open_change: move |open: bool| if !open && !busy() { on_close.call(()); },
            DialogTitle { "删除这条记录？" }
            DialogDescription { "原文和关联凭据会被删除，相关知识将不再用于召回。此操作无法撤销。" }
            p { class: "memory-delete-title", "{source.title}" }
            if let Some(message) = error() { StatusMessage { message, error: true } }
            footer { class: "memory-dialog__footer",
                Button { variant: ButtonVariant::Outline, disabled: busy(), onclick: move |_| on_close.call(()), "取消" }
                Button { variant: ButtonVariant::Destructive, disabled: busy(), onclick: move |_| {
                    if busy() { return; } busy.set(true); error.set(None); let id = source.id.clone();
                    spawn(async move {
                        match transport::request::<Value>("DELETE", &format!("/sources/{id}"), Value::Null).await {
                            Ok(_) => {
                                notes.write().revision += 1;
                                state.write().selected = None;
                                state::refresh(state).await;
                                busy.set(false);
                                on_deleted.call(());
                                return;
                            },
                            Err(message) => error.set(Some(message)),
                        }
                        busy.set(false);
                    });
                }, Trash2 { size: 16 } if busy() { "删除中" } else { "确认删除" } }
            }
        }
    }
}
