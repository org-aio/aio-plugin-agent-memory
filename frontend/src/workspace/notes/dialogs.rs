use super::{NotesState, attachments::AttachmentViewer, item::time_label};
use crate::{
    state::{self, MemoryState},
    transport,
};
use az_memory_model::{RevealedSecret, ShareLink, SourceView};
use az_ui_components::{
    admin::{RequestState, StatusMessage},
    button::{Button, ButtonSize, ButtonVariant},
    dialog::{Dialog, DialogDescription, DialogTitle},
    markdown::Markdown,
    textarea::Textarea,
};
use dioxus::prelude::*;
use dioxus_icons::lucide::{Copy, Pencil, Share2, Trash2, X};
use serde_json::{Value, json};

#[component]
pub(super) fn NoteDetail(
    id: String,
    on_close: EventHandler<()>,
    on_edit: EventHandler<SourceView>,
    on_share: EventHandler<SourceView>,
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
                    div { class: "memory-dialog__body",
                        Markdown { source: source.text.clone(), link_base: "/", image_base: "/" }
                        AttachmentViewer { id: source.id.clone(), attachments: source.attachments.clone() }
                    }
                    footer { class: "memory-dialog__footer",
                        time { "{time_label(source.updated_at)}" }
                        if source.can_delete { Button { variant: ButtonVariant::Outline, onclick: { let source = source.clone(); move |_| on_share.call(source.clone()) }, Share2 { size: 16 } "分享外链" } }
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

/// 外链只读取净化后的正文和图片；令牌和有效期都由服务端保存。
#[component]
pub(super) fn ShareDialog(source: SourceView, on_close: EventHandler<()>) -> Element {
    let load_id = source.id.clone();
    let revoke_id = source.id.clone();
    let create_id = source.id.clone();
    let mut link = use_signal(|| None::<ShareLink>);
    let mut loading = use_signal(|| true);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut copied = use_signal(|| false);
    let mut days = use_signal(|| "1".to_owned());
    use_future(move || {
        let id = load_id.clone();
        async move {
            match transport::request::<Option<ShareLink>>(
                "GET",
                &format!("/sources/{id}/share"),
                Value::Null,
            )
            .await
            {
                Ok(value) => {
                    // 已有链接按剩余有效期预填，重新生成时不必重新换算天数。
                    if let Some(current) = &value {
                        let remaining = (current.expires_at - now_millis()).max(1);
                        days.set((remaining / 86_400_000).clamp(1, 365).to_string());
                    }
                    link.set(value);
                }
                Err(message) => error.set(Some(message)),
            }
            loading.set(false);
        }
    });
    let current = link.read().clone();
    let url = current.as_ref().map(|value| absolute_url(&value.path));
    rsx! {
        Dialog { class: "memory-dialog memory-share", open: true, on_open_change: move |open: bool| if !open && !busy() { on_close.call(()); },
            header { class: "memory-dialog__heading", DialogTitle { "分享这条笔记" }
                Button { size: ButtonSize::IconSm, variant: ButtonVariant::Ghost, title: "关闭", aria_label: "关闭分享", disabled: busy(), onclick: move |_| on_close.call(()), X { size: 18 } }
            }
            DialogDescription { "生成无需登录即可阅读的外链；默认有效期 1 天，可设置 1 到 365 天。" }
            p { class: "memory-share__note", "外链只包含净化后的正文和图片，不包含原文中的保密字段。" }
            if loading() { RequestState {} }
            if let Some(message) = error() { StatusMessage { message, error: true } }
            if !loading() {
                if let Some(current) = current.clone() {
                    div { class: "memory-share__current",
                        label { "当前链接" }
                        input { class: "memory-share__link", r#type: "text", readonly: true, value: url.clone().unwrap_or_default(), aria_label: "分享链接" }
                        p { class: "memory-share__expiry", "有效期至 {time_label(current.expires_at)}" }
                    }
                }
                div { class: "memory-share__days",
                    label { r#for: "memory-share-days", "有效期（天）" }
                    input { id: "memory-share-days", class: "memory-share__days-input", r#type: "number", min: "1", max: "365", value: "{days()}",
                        disabled: busy(), oninput: move |event: FormEvent| days.set(event.value()) }
                }
            }
            footer { class: "memory-dialog__footer",
                if let Some(current) = current.clone() {
                    Button { variant: ButtonVariant::Ghost, disabled: busy(), onclick: move |_| {
                        let value = absolute_url(&current.path);
                        spawn(async move {
                            match transport::copy(value).await {
                                Ok(()) => copied.set(true),
                                Err(message) => error.set(Some(message)),
                            }
                        });
                    }, Copy { size: 16 } if copied() { "已复制" } else { "复制链接" } }
                    Button { variant: ButtonVariant::Destructive, disabled: busy(), onclick: move |_| {
                        if busy() { return; }
                        busy.set(true); error.set(None); copied.set(false); let id = revoke_id.clone();
                        spawn(async move {
                            match transport::request::<Value>("DELETE", &format!("/sources/{id}/share"), Value::Null).await {
                                Ok(_) => { link.set(None); busy.set(false); }
                                Err(message) => { error.set(Some(message)); busy.set(false); }
                            }
                        });
                    }, "停止分享" }
                }
                Button { variant: ButtonVariant::Outline, disabled: busy(), onclick: move |_| on_close.call(()), "关闭" }
                Button { disabled: busy() || loading(), onclick: move |_| {
                    if busy() || loading() { return; }
                    let value = days().trim().parse::<i64>().ok();
                    if !matches!(value, Some(1..=365)) { error.set(Some("有效期请输入 1 到 365 天".into())); return; }
                    busy.set(true); error.set(None); copied.set(false);
                    let id = create_id.clone();
                    let expires = value.unwrap_or(1) * 86_400;
                    spawn(async move {
                        match transport::request::<ShareLink>("POST", &format!("/sources/{id}/share"), json!({"expiresInSeconds": expires})).await {
                            Ok(value) => { link.set(Some(value)); busy.set(false); }
                            Err(message) => { error.set(Some(message)); busy.set(false); }
                        }
                    });
                }, if current.is_some() { "重新生成" } else { "生成链接" } }
            }
        }
    }
}

fn now_millis() -> i64 {
    js_sys::Date::now() as i64
}

fn absolute_url(path: &str) -> String {
    let origin = web_sys::window()
        .and_then(|window| window.location().origin().ok())
        .unwrap_or_default();
    format!("{origin}{path}")
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
