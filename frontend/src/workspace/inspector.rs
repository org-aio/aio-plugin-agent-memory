use crate::state::{self, MemoryState};
use az_ui_components::{
    badge::{Badge, BadgeVariant},
    button::{Button, ButtonSize, ButtonVariant},
};
use dioxus::prelude::*;
use dioxus_icons::lucide::{Pencil, RotateCcw, Trash2};

#[component]
pub fn NodeInspector(on_create: EventHandler<()>) -> Element {
    let state = use_context::<Signal<MemoryState>>();
    let mut edit = use_signal(|| false);
    let selected = state.read().selected.clone();
    let Some(node) = selected else {
        return rsx! {
            aside { style: "padding:16px;border:1px dashed var(--border-color);border-radius:8px;color:var(--muted-foreground);",
                p { "选择一个节点查看详情。" }
                Button { variant: ButtonVariant::Outline, onclick: move |_| on_create.call(()), "新建条目" }
            }
        };
    };
    rsx! {
        aside { style: "padding:16px;border:1px solid var(--border-color);border-radius:8px;background:var(--card-background);position:sticky;top:12px;",
            div { style: "display:flex;align-items:center;gap:8px;justify-content:space-between;",
                h2 { "{node.title}" }
                Badge { variant: BadgeVariant::Outline, "{node.kind.label()}" }
            }
            dl { style: "display:grid;gap:10px;margin-top:12px;",
                if !node.aliases.is_empty() {
                    div { dt { "别名" } dd { "{node.aliases.join(\"、\")}" } }
                }
                div { dt { "版本" } dd { "{node.version}" } }
                if !node.url.is_empty() {
                    div { dt { "来源地址" } dd { a { href: "{node.url}", target: "_blank", "{node.url}" } } }
                }
                if !node.tags.is_empty() {
                    div { dt { "标签" } dd { "{tag_text(&node.tags)}" } }
                }
                div { dt { "正文" } dd { pre { style: "white-space:pre-wrap;", "{node.content}" } } }
            }
            div { style: "display:flex;gap:8px;margin-top:12px;flex-wrap:wrap;",
                Button { size: ButtonSize::Sm, onclick: move |_| edit.set(true), Pencil { size: "14px" } "编辑" }
                Button {
                    size: ButtonSize::Sm,
                    variant: ButtonVariant::Destructive,
                    onclick: {
                        let id = node.id.clone();
                        move |_| {
                            let id = id.clone();
                            spawn(async move { let _ = state::delete_node(state, id).await; });
                        }
                    },
                    Trash2 { size: "14px" }
                    "删除"
                }
            }
            if !state.read().revisions.is_empty() {
                h3 { style: "margin-top:16px;", "历史版本" }
                div { style: "display:grid;gap:6px;",
                    for revision in state.read().revisions.clone().into_iter().take(8) {
                        div { style: "display:flex;justify-content:space-between;align-items:center;gap:8px;",
                            span { "v{revision.version} · {revision.author_type}" }
                            Button {
                                size: ButtonSize::Sm,
                                variant: ButtonVariant::Ghost,
                                onclick: {
                                    let id = node.id.clone();
                                    move |_| {
                                        let id = id.clone();
                                        let version = revision.version;
                                        let current = node.version;
                                        spawn(async move { state::rollback(state, id, version, current).await; });
                                    }
                                },
                                RotateCcw { size: "14px" }
                                "回退"
                            }
                        }
                    }
                }
            }
        }
        if edit() {
            super::dialogs::EditNodeDialog { node: node.clone(), on_close: move |_| edit.set(false) }
        }
    }
}

fn tag_text(tags: &[String]) -> String {
    tags.iter()
        .map(|tag| format!("#{tag}"))
        .collect::<Vec<_>>()
        .join(" ")
}
