use crate::state::{self, MemoryState};
use az_ui_components::badge::{Badge, BadgeVariant};
use dioxus::prelude::*;

#[component]
pub fn GraphPanel() -> Element {
    let state = use_context::<Signal<MemoryState>>();
    let graph = state.read().graph.clone();
    if graph.nodes.is_empty() {
        return rsx! { p { "暂无图谱节点" } };
    }
    let positions = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let seed = node.id.bytes().fold(index as u64, |acc, value| {
                acc.wrapping_mul(31).wrapping_add(value as u64)
            });
            let x = 8.0 + (seed % 84) as f64;
            let y = 8.0 + ((seed / 97) % 76) as f64;
            (node.id.clone(), (x, y))
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    rsx! {
        section { style: "display:grid;gap:12px;",
            div { style: "display:flex;align-items:center;justify-content:space-between;",
                h2 { "图谱" }
                div { style: "display:flex;gap:8px;",
                    Badge { variant: BadgeVariant::Outline, "{graph.nodes.len()} 节点" }
                    Badge { variant: BadgeVariant::Outline, "{graph.edges.len()} 关系" }
                }
            }
            div { style: "position:relative;height:520px;border:1px solid var(--border-color);border-radius:8px;background:var(--card-background);overflow:hidden;",
                svg { view_box: "0 0 100 100", preserve_aspect_ratio: "none", style: "position:absolute;inset:0;width:100%;height:100%;",
                    for edge in graph.edges.iter() {
                        if let (Some((x1, y1)), Some((x2, y2))) = (positions.get(&edge.source), positions.get(&edge.target)) {
                            line {
                                key: "{edge.id}",
                                x1: "{x1}",
                                y1: "{y1}",
                                x2: "{x2}",
                                y2: "{y2}",
                                stroke: "var(--border-color)",
                                stroke_width: "0.25",
                            }
                        }
                    }
                }
                for node in graph.nodes.iter() {
                    if let Some((x, y)) = positions.get(&node.id) {
                        button {
                            key: "{node.id}",
                            style: "position:absolute;left:{x}%;top:{y}%;transform:translate(-50%,-50%);max-width:160px;padding:6px 8px;border:1px solid var(--border-color);border-radius:999px;background:var(--background);cursor:pointer;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;",
                            title: "{node.title}",
                            onclick: {
                                let id = node.id.clone();
                                move |_| {
                                    let id = id.clone();
                                    spawn(async move { state::open_node(state, id).await; });
                                }
                            },
                            "{node.title}"
                        }
                    }
                }
            }
        }
    }
}
