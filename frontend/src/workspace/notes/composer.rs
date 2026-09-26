use super::{Draft, NotesState};
use crate::{
    state::{self, MemoryState},
    transport,
};
use az_memory_model::SourceView;
use az_ui_components::{
    admin::StatusMessage,
    button::{Button, ButtonVariant},
    markdown::Markdown,
    textarea::Textarea,
};
use dioxus::prelude::*;
use dioxus_icons::lucide::{ArrowUp, PenLine};

#[component]
pub(super) fn Composer(on_saved: EventHandler<()>) -> Element {
    let state = use_context::<Signal<MemoryState>>();
    let mut notes = use_context::<Signal<NotesState>>();
    let mut composing = use_signal(|| false);
    let mut preview = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut saved = use_signal(|| false);
    let space = state.read().space_id.clone().unwrap_or_default();
    let draft = notes.read().drafts.get(&space).cloned().unwrap_or_default();
    let can_write = state
        .read()
        .spaces
        .iter()
        .any(|s| s.id == space && s.role.can_write());
    let saving = notes.read().saving;
    let can_save = can_write && !saving && !draft.text.trim().is_empty();
    let save = Callback::new({
        let space = space.clone();
        move |_: ()| {
            if notes.peek().saving || !can_write {
                return;
            }
            let Some(draft) = notes.peek().drafts.get(&space).cloned() else {
                return;
            };
            if draft.text.trim().is_empty() {
                return;
            }
            if draft.text.len() > 100_000 {
                error.set(Some("正文不能超过 100 KB".into()));
                return;
            }
            notes.write().saving = true;
            error.set(None);
            saved.set(false);
            let space = space.clone();
            spawn(async move {
                let body = serde_json::json!({ "requestId": draft.request_id, "text": draft.text, "spaceId": space, "origin": "note" });
                match transport::request::<SourceView>("POST", "/capture", body).await {
                    Ok(_) => {
                        notes.write().drafts.remove(&space);
                        notes.write().revision += 1;
                        on_saved.call(());
                        preview.set(false);
                        saved.set(true);
                        state::refresh(state).await;
                    }
                    Err(message) => error.set(Some(message)),
                }
                notes.write().saving = false;
            });
        }
    });
    rsx! {
        section { class: "memory-composer", aria_label: "随心记",
            header { class: "memory-composer__heading", PenLine { size: 18 } h2 { "随心记" } }
            form { onsubmit: move |event: FormEvent| { event.prevent_default(); save.call(()); },
                if preview() {
                    div { class: "memory-composer__preview", Markdown { source: draft.text.clone(), link_base: "/", image_base: "/" } }
                } else {
                    Textarea { aria_label: "随心记内容", placeholder: if can_write { "此刻想记下什么？" } else { "当前空间只读" },
                        rows: "4", value: draft.text.clone(), disabled: saving || !can_write,
                        oncompositionstart: move |_| composing.set(true), oncompositionend: move |_| composing.set(false),
                        oninput: { let space = space.clone(); move |event: FormEvent| {
                            let mut value = notes.write(); let draft = value.drafts.entry(space.clone()).or_insert_with(Draft::default);
                            // 内容变化后才换幂等键，网络失败原样重试不会重复收件。
                            draft.request_id = uuid::Uuid::new_v4().to_string(); draft.text = event.value(); saved.set(false);
                        } },
                        onkeydown: move |event: KeyboardEvent| {
                            if event.key() == Key::Enter && (event.modifiers().meta() || event.modifiers().ctrl()) && !composing() && !event.is_composing() {
                                event.prevent_default(); save.call(());
                            }
                        },
                    }
                }
                footer { class: "memory-composer__footer",
                    div { class: "memory-composer__modes", role: "group", aria_label: "输入模式",
                        Button { r#type: "button", variant: ButtonVariant::Ghost, aria_pressed: !preview(), onclick: move |_| preview.set(false), "编辑" }
                        Button { r#type: "button", variant: ButtonVariant::Ghost, aria_pressed: preview(), onclick: move |_| preview.set(true), "预览" }
                    }
                    if saved() { span { class: "memory-saved", role: "status", "已记下" } }
                    Button { r#type: "submit", title: "保存 (Ctrl / Command + Enter)", disabled: !can_save, aria_label: "保存随心记",
                        ArrowUp { size: 16 } if saving { "保存中" } else { "记下" }
                    }
                }
            }
            if let Some(message) = error() { StatusMessage { message, error: true } }
        }
    }
}
