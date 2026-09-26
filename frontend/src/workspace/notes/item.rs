use super::super::source_status;
use az_memory_model::SourceView;
use az_ui_components::{
    button::{Button, ButtonSize, ButtonVariant},
    markdown::Markdown,
};
use dioxus::prelude::*;
use dioxus_icons::lucide::{Expand, Pencil, Trash2};

pub(super) fn time_label(timestamp: i64) -> String {
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(timestamp as f64));
    format!(
        "{}-{:02}-{:02} {:02}:{:02}",
        date.get_full_year(),
        date.get_month() + 1,
        date.get_date(),
        date.get_hours(),
        date.get_minutes()
    )
}

#[component]
pub(super) fn NoteItem(
    source: SourceView,
    on_open: EventHandler<String>,
    on_edit: EventHandler<SourceView>,
    on_delete: EventHandler<SourceView>,
) -> Element {
    let time = time_label(source.updated_at);
    let origin = match source.origin.as_str() {
        "chat" => "对话",
        "import" => "导入",
        _ => "随心记",
    };
    rsx! {
        article { class: "memory-note", "data-note-id": source.id.clone(),
            header { class: "memory-note__meta",
                time { "{time}" } span { class: "memory-note__origin", "{origin}" }
                span { class: "memory-note__status", "data-status": source.status.clone(), "{source_status(&source)}" }
                div { class: "memory-note__actions",
                    Button { size: ButtonSize::IconSm, variant: ButtonVariant::Ghost, title: "查看详情", aria_label: "查看详情",
                        onclick: { let id = source.id.clone(); move |_| on_open.call(id.clone()) }, Expand { size: 15 } }
                    if source.can_edit {
                        Button { size: ButtonSize::IconSm, variant: ButtonVariant::Ghost, title: "编辑记录", aria_label: "编辑记录",
                            onclick: { let source = source.clone(); move |_| on_edit.call(source.clone()) }, Pencil { size: 15 } }
                    }
                    if source.can_delete {
                        Button { size: ButtonSize::IconSm, variant: ButtonVariant::Ghost, title: "删除记录", aria_label: "删除记录",
                            onclick: { let source = source.clone(); move |_| on_delete.call(source.clone()) }, Trash2 { size: 15 } }
                    }
                }
            }
            div { class: "memory-note__preview", Markdown { source: source.text.clone(), link_base: "/", image_base: "/" } }
            footer { class: "memory-note__footer",
                Button { size: ButtonSize::Sm, variant: ButtonVariant::Link,
                    onclick: { let id = source.id.clone(); move |_| on_open.call(id.clone()) }, "阅读全文" }
            }
        }
    }
}
