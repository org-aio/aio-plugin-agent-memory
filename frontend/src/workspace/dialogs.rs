use crate::state::{self, MemoryState};
use az_memory_model::{MemoryNode, NodeDraft, NodeKind};
use az_ui_components::{admin::EditorDialog, input::TextInput, textarea::Textarea};
use dioxus::prelude::*;

#[component]
pub fn NodeDialog(on_close: EventHandler<()>) -> Element {
    let state = use_context::<Signal<MemoryState>>();
    let mut title = use_signal(String::new);
    let mut content = use_signal(String::new);
    let mut url = use_signal(String::new);
    let mut tags = use_signal(String::new);
    let mut aliases = use_signal(String::new);
    let kind = use_signal(|| NodeKind::Note);
    rsx! {
        EditorDialog {
            title: "新建条目",
            description: "保存到当前知识空间。",
            on_close,
            on_saved: move |_| on_close.call(()),
            save: move |_| {
                Box::pin(async move {
                    state::save_node(state, None, draft(
                        title(),
                        kind(),
                        content(),
                        url(),
                        tags(),
                        aliases(),
                        None,
                    )).await?;
                    Ok(())
                }) as az_ui_components::admin::AsyncResult<()>
            },
            KindField { kind }
            TextInput { label: "标题", value: title(), on_change: move |value| title.set(value) }
            label { "正文" Textarea { value: content(), oninput: move |event: FormEvent| content.set(event.value()) } }
            TextInput { label: "来源地址", value: url(), on_change: move |value| url.set(value) }
            TextInput { label: "标签", value: tags(), on_change: move |value| tags.set(value), placeholder: Some("用逗号分隔".into()) }
            TextInput { label: "别名", value: aliases(), on_change: move |value| aliases.set(value), placeholder: Some("用逗号分隔".into()) }
        }
    }
}

#[component]
pub fn EditNodeDialog(node: MemoryNode, on_close: EventHandler<()>) -> Element {
    let state = use_context::<Signal<MemoryState>>();
    let mut title = use_signal(|| node.title.clone());
    let mut content = use_signal(|| node.content.clone());
    let mut url = use_signal(|| node.url.clone());
    let mut tags = use_signal(|| node.tags.join(", "));
    let mut aliases = use_signal(|| node.aliases.join(", "));
    let kind = use_signal(|| node.kind);
    let id = node.id.clone();
    let version = node.version;
    rsx! {
        EditorDialog {
            title: "编辑条目",
            description: "过期版本会被拒绝。",
            on_close,
            on_saved: move |_| on_close.call(()),
            save: move |_| {
                let id = id.clone();
                Box::pin(async move {
                    state::save_node(state, Some(id), draft(
                        title(),
                        kind(),
                        content(),
                        url(),
                        tags(),
                        aliases(),
                        Some(version),
                    )).await?;
                    Ok(())
                }) as az_ui_components::admin::AsyncResult<()>
            },
            KindField { kind }
            TextInput { label: "标题", value: title(), on_change: move |value| title.set(value) }
            label { "正文" Textarea { value: content(), oninput: move |event: FormEvent| content.set(event.value()) } }
            TextInput { label: "来源地址", value: url(), on_change: move |value| url.set(value) }
            TextInput { label: "标签", value: tags(), on_change: move |value| tags.set(value), placeholder: Some("用逗号分隔".into()) }
            TextInput { label: "别名", value: aliases(), on_change: move |value| aliases.set(value), placeholder: Some("用逗号分隔".into()) }
        }
    }
}

#[component]
fn KindField(kind: Signal<NodeKind>) -> Element {
    let mut kind = kind;
    rsx! {
        label { "类型"
            select {
                value: "{kind():?}",
                onchange: move |event| {
                    kind.set(match event.value().as_str() {
                        "Concept" => NodeKind::Concept,
                        "Person" => NodeKind::Person,
                        "Event" => NodeKind::Event,
                        "Project" => NodeKind::Project,
                        _ => NodeKind::Note,
                    });
                },
                for (label, value) in state::kind_options() {
                    option { value: "{value:?}", "{label}" }
                }
            }
        }
    }
}

#[component]
pub fn SourceDialog(on_close: EventHandler<()>) -> Element {
    let state = use_context::<Signal<MemoryState>>();
    let mut title = use_signal(String::new);
    let mut url = use_signal(String::new);
    let mut text = use_signal(String::new);
    rsx! {
        EditorDialog {
            title: "导入资料",
            description: "资料会经过秘密隔离后保存。",
            on_close,
            on_saved: move |_| on_close.call(()),
            save: move |_| {
                Box::pin(async move {
                    if text().trim().is_empty() {
                        return Err("资料正文不能为空".into());
                    }
                    state::capture(state, title(), text(), url()).await
                }) as az_ui_components::admin::AsyncResult<()>
            },
            TextInput { label: "标题", value: title(), on_change: move |value| title.set(value) }
            TextInput { label: "来源地址", value: url(), on_change: move |value| url.set(value) }
            label { "资料正文" Textarea { value: text(), oninput: move |event: FormEvent| text.set(event.value()) } }
        }
    }
}

fn draft(
    title: String,
    kind: NodeKind,
    content: String,
    url: String,
    tags: String,
    aliases: String,
    version: Option<i64>,
) -> NodeDraft {
    NodeDraft {
        title,
        kind,
        content,
        url,
        tags: split(tags),
        version,
        aliases: split(aliases),
    }
}

fn split(value: String) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}
