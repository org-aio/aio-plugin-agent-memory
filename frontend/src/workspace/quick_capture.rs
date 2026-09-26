use super::source_status;
use crate::state::{self, MemoryState};
use az_memory_model::SourceView;
use az_ui_components::{
    admin::EmptyState,
    badge::{Badge, BadgeVariant},
    button::{Button, ButtonSize, ButtonVariant},
    textarea::Textarea,
};
use dioxus::prelude::*;
use dioxus_icons::lucide::{ArrowUp, Clock3, FileText, Search};

#[component]
pub fn QuickCapture() -> Element {
    let mut state = use_context::<Signal<MemoryState>>();
    let mut draft = use_signal(String::new);
    let mut composing = use_signal(|| false);
    let mut saving = use_signal(|| false);
    let can_save = !draft().trim().is_empty() && !saving();
    let space_title = state
        .read()
        .spaces
        .iter()
        .find(|space| Some(space.id.as_str()) == state.read().space_id.as_deref())
        .map(|space| space.title.clone())
        .unwrap_or_else(|| "个人空间".into());
    let recent = state
        .read()
        .sources
        .iter()
        .take(12)
        .cloned()
        .collect::<Vec<_>>();

    let mut save = move || {
        if !can_save {
            return;
        }
        let text = draft().trim().to_owned();
        saving.set(true);
        spawn(async move {
            if state::capture(state, String::new(), text, String::new())
                .await
                .is_ok()
            {
                draft.set(String::new());
            }
            saving.set(false);
        });
    };

    rsx! {
        div { style: "display:grid;gap:24px;max-width:980px;margin:0 auto;width:100%;",
            section { style: "display:grid;gap:10px;",
                div { style: "display:flex;align-items:end;justify-content:space-between;gap:16px;flex-wrap:wrap;",
                    div {
                        h2 { style: "margin:0;font-size:22px;font-weight:650;letter-spacing:0;", "现在想到什么？" }
                        p { style: "margin:6px 0 0;color:var(--muted-foreground);", "先记下来，稍后再整理。" }
                    }
                    Badge { variant: BadgeVariant::Outline, "{space_title}" }
                }
                form {
                    style: "display:grid;gap:10px;padding:12px;border:1px solid var(--border-color);border-radius:8px;background:var(--card-background);",
                    onsubmit: move |event: FormEvent| {
                        event.prevent_default();
                        save();
                    },
                    Textarea {
                        aria_label: "随心记内容",
                        placeholder: "随手写点东西……标题、正文、链接都可以。",
                        rows: "7",
                        value: draft(),
                        disabled: saving(),
                        oncompositionstart: move |_| composing.set(true),
                        oncompositionend: move |_| composing.set(false),
                        oninput: move |event: FormEvent| draft.set(event.value()),
                        onkeydown: move |event: KeyboardEvent| {
                            let command_enter = event.key() == Key::Enter
                                && (event.modifiers().meta() || event.modifiers().ctrl())
                                && !composing()
                                && !event.is_composing();
                            if command_enter {
                                event.prevent_default();
                                save();
                            }
                        },
                    }
                    div { style: "display:flex;align-items:center;justify-content:space-between;gap:12px;flex-wrap:wrap;",
                        span { style: "font-size:12px;color:var(--muted-foreground);", "⌘/Ctrl + Enter 保存" }
                        Button {
                            r#type: "submit",
                            disabled: !can_save,
                            aria_label: "保存随心记",
                            ArrowUp { size: "16px" }
                            if saving() { "保存中" } else { "记下" }
                        }
                    }
                }
            }
            section { style: "display:grid;gap:12px;",
                div { style: "display:flex;align-items:center;justify-content:space-between;gap:12px;",
                    div { style: "display:flex;align-items:center;gap:8px;",
                        Clock3 { size: "18px" }
                        h2 { style: "margin:0;font-size:16px;font-weight:650;", "最近记录" }
                    }
                    Button {
                        size: ButtonSize::Sm,
                        variant: ButtonVariant::Ghost,
                        onclick: move |_| state.write().view = Some(crate::state::View::Sources),
                        Search { size: "14px" }
                        "全部"
                    }
                }
                if recent.is_empty() {
                    EmptyState { title: "还没有记录", detail: "上方输入第一条内容，之后可以继续整理成 Wiki 或知识图谱。" }
                }
                div { style: "display:grid;grid-template-columns:repeat(auto-fill,minmax(260px,1fr));gap:10px;",
                    for source in recent {
                        QuickSource { source }
                    }
                }
            }
        }
    }
}

#[component]
fn QuickSource(source: SourceView) -> Element {
    let preview = source.text.trim().replace('\n', " ");
    let title = preview.chars().take(34).collect::<String>();
    let title = if preview.chars().count() > 34 {
        format!("{title}…")
    } else {
        title
    };
    rsx! {
        article {
            style: "min-width:0;padding:12px;border:1px solid var(--border-color);border-radius:8px;background:var(--background);",
            div { style: "display:flex;align-items:flex-start;justify-content:space-between;gap:10px;",
                div { style: "display:flex;align-items:center;gap:8px;min-width:0;",
                    FileText { size: "15px" }
                    strong { style: "overflow:hidden;text-overflow:ellipsis;white-space:nowrap;", "{title}" }
                }
                Badge { variant: BadgeVariant::Outline, "{source_status(&source)}" }
            }
            p { style: "margin:8px 0 0;color:var(--muted-foreground);display:-webkit-box;-webkit-line-clamp:3;-webkit-box-orient:vertical;overflow:hidden;", "{preview}" }
        }
    }
}
