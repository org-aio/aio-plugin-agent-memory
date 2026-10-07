use crate::state::{self, MemoryState};
use az_memory_model::{MemoryEdge, MemoryNode};
use az_ui_components::{
    admin::EmptyState,
    badge::{Badge, BadgeVariant},
    button::{Button, ButtonSize, ButtonVariant},
};
use dioxus::prelude::*;
use dioxus_icons::lucide::{Maximize2, Minus, Plus, RefreshCw};
use std::collections::{BTreeMap, BTreeSet};

/// 关系图的布局画布尺寸；视口按比例缩放，节点坐标始终落在这个范围内。
const SCENE_WIDTH: f64 = 1000.0;
const SCENE_HEIGHT: f64 = 700.0;
const MIN_ZOOM: f64 = 0.4;
const MAX_ZOOM: f64 = 3.0;

/// 关系图视图：绘制知识节点之间的关系，支持平移、缩放和拖动节点。
///
/// 点击节点会加载节点详情并在右侧列出它的关系；没有关系时给出明确提示，
/// 而不是把节点堆成一片标签。
#[component]
pub fn GraphPanel() -> Element {
    let mut state = use_context::<Signal<MemoryState>>();
    let graph = state.read().overview.clone();
    let mut selected = use_signal(|| None::<String>);
    let mut zoom = use_signal(|| 1.0f64);
    let mut center = use_signal(|| (SCENE_WIDTH / 2.0, SCENE_HEIGHT / 2.0));
    let mut size = use_signal(|| (0.0f64, 0.0f64));
    // 拖动节点保存抓取偏移，平移视图保存起点，指针移动时才能保持相对位置。
    let mut dragging = use_signal(|| None::<(String, f64, f64)>);
    let mut panning = use_signal(|| None::<(f64, f64, f64, f64)>);

    // 相同数据得到相同布局，避免每次渲染节点乱跳。
    let nodes = graph.nodes.clone();
    let edges = graph.edges.clone();
    let layout_nodes = nodes.clone();
    let layout_edges = edges.clone();
    let computed = use_memo(use_reactive!(|(layout_nodes, layout_edges)| layout(
        &layout_nodes,
        &layout_edges
    )));
    // 拖动只覆盖被移动的节点；图数据变化时清空覆盖，回到新布局。
    let mut moved = use_signal(BTreeMap::<String, (f64, f64)>::new);
    use_effect(move || {
        computed();
        moved.set(BTreeMap::new());
    });

    let selected_id = selected();
    let neighbor_edges = edges.clone();
    let neighbors = use_memo(use_reactive!(
        |(neighbor_edges, selected_id)| match &selected_id {
            Some(id) => related_ids(&neighbor_edges, id),
            None => BTreeSet::new(),
        }
    ));
    let title_nodes = nodes.clone();
    let titles = use_memo(use_reactive!(|(title_nodes)| title_nodes
        .iter()
        .map(|node| (node.id.clone(), node.title.clone()))
        .collect::<BTreeMap<_, _>>()));

    if graph.nodes.is_empty() {
        return rsx! {
            EmptyState {
                title: "暂无图谱节点",
                detail: "整理完的条目才会进入关系图；先记录内容，等后台整理完成。",
            }
        };
    }

    let selected_node =
        selected().and_then(|id| graph.nodes.iter().find(|node| node.id == id).cloned());
    let relation_count = graph.edges.len();
    rsx! {
        section { class: "memory-graph",
            div { class: "memory-graph__toolbar",
                h2 { "关系图" }
                div { class: "memory-graph__badges",
                    Badge { variant: BadgeVariant::Outline, "{graph.nodes.len()} 个知识节点" }
                    Badge { variant: BadgeVariant::Outline, "{relation_count} 条关系" }
                    if graph.truncated {
                        Badge { variant: BadgeVariant::Outline, "已截断" }
                    }
                }
                div { class: "memory-graph__actions",
                    Button {
                        variant: ButtonVariant::Outline, size: ButtonSize::IconSm,
                        title: "刷新", aria_label: "刷新图谱",
                        disabled: state.read().busy,
                        onclick: move |_| {
                            selected.set(None);
                            spawn(async move { state::refresh_overview(state).await; });
                        },
                        RefreshCw { size: "16px" }
                    }
                    Button {
                        variant: ButtonVariant::Outline, size: ButtonSize::IconSm,
                        title: "缩小", aria_label: "缩小图谱",
                        onclick: move |_| zoom.set((zoom() / 1.25).max(MIN_ZOOM)),
                        Minus { size: "16px" }
                    }
                    Button {
                        variant: ButtonVariant::Outline, size: ButtonSize::IconSm,
                        title: "放大", aria_label: "放大图谱",
                        onclick: move |_| zoom.set((zoom() * 1.25).min(MAX_ZOOM)),
                        Plus { size: "16px" }
                    }
                    Button {
                        variant: ButtonVariant::Outline, size: ButtonSize::IconSm,
                        title: "重置视图", aria_label: "重置图谱视图",
                        onclick: move |_| {
                            zoom.set(1.0);
                            center.set((SCENE_WIDTH / 2.0, SCENE_HEIGHT / 2.0));
                            moved.set(BTreeMap::new());
                        },
                        Maximize2 { size: "16px" }
                    }
                }
            }
            if relation_count == 0 {
                p { class: "memory-graph__hint", role: "status",
                    "当前空间的条目之间还没有关系。整理资料时模型会提取关系，也可以手动补充。"
                }
            }
            div { class: "memory-graph__body",
                div { class: "memory-graph__canvas",
                    svg {
                        class: "memory-graph__svg",
                        view_box: view_box(center(), zoom()),
                        preserve_aspect_ratio: "xMidYMid meet",
                        "role": "img",
                        "aria-label": "记忆关系图",
                        onmounted: move |event: MountedEvent| {
                            let data = event.data();
                            spawn(async move {
                                if let Ok(rect) = data.get_client_rect().await {
                                    size.set((rect.width(), rect.height()));
                                }
                            });
                        },
                        onpointerdown: move |event: PointerEvent| {
                            let point =
                                scene_point(element_point(&*event.data()), size(), center(), zoom());
                            let hit = hit_node(
                                element_point(&*event.data()),
                                size(),
                                center(),
                                zoom(),
                                &graph.nodes,
                                &moved(),
                                &computed(),
                            );
                            match hit {
                                Some(id) => {
                                    let position =
                                        position_of(&id, &moved(), &computed()).unwrap_or(point);
                                    dragging.set(Some((
                                        id.clone(),
                                        position.0 - point.0,
                                        position.1 - point.1,
                                    )));
                                    selected.set(Some(id));
                                }
                                None => {
                                    selected.set(None);
                                    panning.set(Some((center().0, center().1, point.0, point.1)));
                                }
                            }
                        },
                        onpointermove: move |event: PointerEvent| {
                            let point =
                                scene_point(element_point(&*event.data()), size(), center(), zoom());
                            if let Some((id, offset_x, offset_y)) = dragging() {
                                moved.write().insert(id, (point.0 + offset_x, point.1 + offset_y));
                            }
                            if let Some((cx, cy, start_x, start_y)) = panning() {
                                center.set((cx - (point.0 - start_x), cy - (point.1 - start_y)));
                            }
                        },
                        onpointerup: move |_| {
                            dragging.set(None);
                            panning.set(None);
                        },
                        onpointerleave: move |_| {
                            dragging.set(None);
                            panning.set(None);
                        },
                        onwheel: move |event: WheelEvent| {
                            event.prevent_default();
                            let step = match event.delta().strip_units().y {
                                value if value < 0.0 => 1.12,
                                _ => 1.0 / 1.12,
                            };
                            let current = zoom();
                            let next = (current * step).clamp(MIN_ZOOM, MAX_ZOOM);
                            let before = scene_point(element_point(&*event.data()), size(), center(), current);
                            let (cx, cy) = center();
                            // 以指针下的画布点为锚点：缩放前后该点始终停在指针位置。
                            let anchor_x = cx + (before.0 - cx) * (1.0 - current / next);
                            let anchor_y = cy + (before.1 - cy) * (1.0 - current / next);
                            zoom.set(next);
                            center.set((anchor_x, anchor_y));
                        },
                        for edge in graph.edges.iter() {
                            if let (Some(from), Some(to)) = (
                                position_of(&edge.source, &moved(), &computed()),
                                position_of(&edge.target, &moved(), &computed()),
                            ) {
                                line {
                                    key: "{edge.id}",
                                    class: "memory-graph__edge",
                                    "data-active": selected().as_deref().is_some_and(|id| id == edge.source || id == edge.target),
                                    x1: "{from.0}", y1: "{from.1}", x2: "{to.0}", y2: "{to.1}",
                                }
                            }
                        }
                        for edge in graph.edges.iter().filter(|edge| !edge.relation.is_empty()) {
                            if let (Some(from), Some(to)) = (
                                position_of(&edge.source, &moved(), &computed()),
                                position_of(&edge.target, &moved(), &computed()),
                            ) {
                                text {
                                    key: "label-{edge.id}",
                                    class: "memory-graph__edge-label",
                                    x: "{(from.0 + to.0) / 2.0}",
                                    y: "{(from.1 + to.1) / 2.0}",
                                    "{edge.relation}"
                                }
                            }
                        }
                        for node in graph.nodes.iter() {
                            if let Some((x, y)) = position_of(&node.id, &moved(), &computed()) {
                                g {
                                    key: "{node.id}",
                                    class: "memory-graph__node",
                                    "data-kind": "{kind_key(node)}",
                                    "data-selected": selected().as_deref() == Some(node.id.as_str()),
                                    "data-dim": selected().as_deref().is_some_and(|id| id != node.id && !neighbors().contains(&node.id)),
                                    "role": "button",
                                    "tabindex": "0",
                                    "aria-label": "{node.title}",
                                    onkeydown: {
                                        let id = node.id.clone();
                                        move |event: KeyboardEvent| {
                                            if event.key() == Key::Enter {
                                                selected.set(Some(id.clone()));
                                                let id = id.clone();
                                                spawn(async move { state::open_node(state, id).await; });
                                            }
                                        }
                                    },
                                    onclick: {
                                        let id = node.id.clone();
                                        move |_| {
                                            selected.set(Some(id.clone()));
                                            let id = id.clone();
                                            spawn(async move { state::open_node(state, id).await; });
                                        }
                                    },
                                    circle { cx: "{x}", cy: "{y}", r: "{radius(degree_of(&graph.edges, &node.id))}" }
                                    text { x: "{x}", y: "{y + radius(degree_of(&graph.edges, &node.id)) + 14.0}", "{truncate(&node.title)}" }
                                }
                            }
                        }
                    }
                }
                aside { class: "memory-graph__aside", aria_label: "节点关系",
                    match selected_node {
                        Some(node) => rsx! {
                            h3 { "{node.title}" }
                            div { class: "memory-graph__meta",
                                Badge { variant: BadgeVariant::Outline, "{node.kind.label()}" }
                                span { "版本 {node.version}" }
                            }
                            if !node.tags.is_empty() {
                                p { class: "memory-graph__tags", "{tag_text(&node.tags)}" }
                            }
                            h4 { "关系" }
                            if graph.edges.iter().any(|edge| edge.source == node.id || edge.target == node.id) {
                                ul { class: "memory-graph__relations",
                                    for edge in graph.edges.iter().filter(|edge| edge.source == node.id || edge.target == node.id) {
                                        {
                                            let outgoing = edge.source == node.id;
                                            let other = if outgoing { &edge.target } else { &edge.source };
                                            let title = titles().get(other).cloned().unwrap_or_else(|| other.clone());
                                            rsx! {
                                                li { key: "{edge.id}",
                                                    span { class: "memory-graph__direction", if outgoing { "→" } else { "←" } }
                                                    span { class: "memory-graph__relation", "{edge.relation}" }
                                                    span { class: "memory-graph__target", "{title}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            } else {
                                p { class: "memory-graph__hint", "该条目暂时没有关系。" }
                            }
                            Button {
                                size: ButtonSize::Sm,
                                onclick: {
                                    let id = node.id.clone();
                                    move |_| {
                                        state.write().view = Some(state::View::Wiki);
                                        let id = id.clone();
                                        spawn(async move { state::open_node(state, id).await; });
                                    }
                                },
                                "在 Wiki 中查看"
                            }
                        },
                        None => rsx! {
                            p { class: "memory-graph__hint", "选择一个节点查看它的关系。" }
                        },
                    }
                }
            }
            div { class: "memory-graph__legend", aria_label: "节点类型图例",
                for (label, key) in [
                    ("笔记", "note"),
                    ("概念", "concept"),
                    ("人物", "person"),
                    ("事件", "event"),
                    ("项目", "project"),
                ] {
                    span { class: "memory-graph__legend-item",
                        i { class: "memory-graph__dot", "data-kind": "{key}" }
                        "{label}"
                    }
                }
            }
        }
    }
}

/// 视口字符串：以 `center` 为中心、按 `zoom` 截取画布。
fn view_box(center: (f64, f64), zoom: f64) -> String {
    let width = SCENE_WIDTH / zoom;
    let height = SCENE_HEIGHT / zoom;
    format!(
        "{} {} {} {}",
        center.0 - width / 2.0,
        center.1 - height / 2.0,
        width,
        height
    )
}

/// 元素内坐标 → 画布坐标。
///
/// `preserve_aspect_ratio="xMidYMid meet"` 会等比缩放并居中，这里按同样的映射反推，
/// 拖动和缩放才能和指针位置一致。指针事件和滚轮事件都实现同一套坐标 trait。
fn scene_point(element: (f64, f64), size: (f64, f64), center: (f64, f64), zoom: f64) -> (f64, f64) {
    scene_from_element(element.0, element.1, size, center, zoom)
}

/// 读取事件相对元素的坐标；指针事件和滚轮事件都实现同一套坐标 trait。
fn element_point<T: InteractionElementOffset>(data: &T) -> (f64, f64) {
    let point = data.element_coordinates();
    (point.x, point.y)
}

fn scene_from_element(
    ex: f64,
    ey: f64,
    size: (f64, f64),
    center: (f64, f64),
    zoom: f64,
) -> (f64, f64) {
    if size.0 <= 0.0 || size.1 <= 0.0 {
        return center;
    }
    let scale = (size.0 / (SCENE_WIDTH / zoom)).min(size.1 / (SCENE_HEIGHT / zoom));
    (
        (ex - size.0 / 2.0) / scale + center.0,
        (ey - size.1 / 2.0) / scale + center.1,
    )
}

/// 合并拖动覆盖与布局坐标：被拖动过的节点优先，其余使用布局结果。
fn position_of(
    id: &str,
    moved: &BTreeMap<String, (f64, f64)>,
    base: &BTreeMap<String, (f64, f64)>,
) -> Option<(f64, f64)> {
    moved.get(id).copied().or_else(|| base.get(id).copied())
}

/// 命中测试：返回指针下最上层的节点。
fn hit_node(
    element: (f64, f64),
    size: (f64, f64),
    center: (f64, f64),
    zoom: f64,
    nodes: &[MemoryNode],
    moved: &BTreeMap<String, (f64, f64)>,
    base: &BTreeMap<String, (f64, f64)>,
) -> Option<String> {
    let point = scene_point(element, size, center, zoom);
    nodes
        .iter()
        .filter_map(|node| position_of(&node.id, moved, base).map(|position| (node, position)))
        .filter(|(_, position)| {
            let dx = position.0 - point.0;
            let dy = position.1 - point.1;
            dx * dx + dy * dy <= (radius(6) + 4.0).powi(2)
        })
        .min_by(|left, right| {
            let dl = (left.1.0 - point.0).powi(2) + (left.1.1 - point.1).powi(2);
            let dr = (right.1.0 - point.0).powi(2) + (right.1.1 - point.1).powi(2);
            dl.total_cmp(&dr)
        })
        .map(|(node, _)| node.id.clone())
}

/// 确定性力导向布局：相同输入得到相同结果，节点不会在重绘时跳动。
fn layout(nodes: &[MemoryNode], edges: &[MemoryEdge]) -> BTreeMap<String, (f64, f64)> {
    let count = nodes.len();
    if count == 0 {
        return BTreeMap::new();
    }
    let index_of: BTreeMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect();
    let links: Vec<(usize, usize)> = edges
        .iter()
        .filter_map(|edge| {
            Some((
                *index_of.get(edge.source.as_str())?,
                *index_of.get(edge.target.as_str())?,
            ))
        })
        .collect();
    // 黄金角螺旋铺开初始位置，避免所有节点从同一点出发。
    // 节点少时放大起始半径，否则少量节点会缩在画布中央一小块。
    let spread = if count <= 8 { 190.0 } else { 90.0 };
    let mut points: Vec<(f64, f64)> = (0..count)
        .map(|index| {
            let angle = index as f64 * 2.399_963_229_728_653;
            let radius = spread + (index % 9) as f64 * 26.0;
            (
                SCENE_WIDTH / 2.0 + radius * angle.cos(),
                SCENE_HEIGHT / 2.0 + radius * angle.sin(),
            )
        })
        .collect();
    let iterations = if count > 120 { 90 } else { 200 };
    for step in 0..iterations {
        let mut forces = vec![(0.0f64, 0.0f64); count];
        for left in 0..count {
            for right in (left + 1)..count {
                let dx = points[left].0 - points[right].0;
                let dy = points[left].1 - points[right].1;
                let distance = (dx * dx + dy * dy).sqrt().max(1.0);
                let force = 9_000.0 / (distance * distance);
                let ux = dx / distance * force;
                let uy = dy / distance * force;
                forces[left].0 += ux;
                forces[left].1 += uy;
                forces[right].0 -= ux;
                forces[right].1 -= uy;
            }
        }
        for (source, target) in &links {
            let dx = points[*target].0 - points[*source].0;
            let dy = points[*target].1 - points[*source].1;
            let distance = (dx * dx + dy * dy).sqrt().max(1.0);
            let pull = (distance - 150.0) * 0.012;
            let ux = dx / distance * pull;
            let uy = dy / distance * pull;
            forces[*source].0 += ux;
            forces[*source].1 += uy;
            forces[*target].0 -= ux;
            forces[*target].1 -= uy;
        }
        // 迭代后期收敛步长，布局稳定下来。
        let cooling = 1.0 - step as f64 / iterations as f64;
        for index in 0..count {
            let to_center_x = SCENE_WIDTH / 2.0 - points[index].0;
            let to_center_y = SCENE_HEIGHT / 2.0 - points[index].1;
            points[index].0 += (forces[index].0 + to_center_x * 0.006) * cooling;
            points[index].1 += (forces[index].1 + to_center_y * 0.006) * cooling;
        }
    }
    // 力平衡只保证相对结构，节点少时会缩在中间一小块；最后等比铺满画布，
    // 让少量节点也保持可读的间距。
    spread_to_canvas(&mut points);
    nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.clone(), points[index]))
        .collect()
}

/// 把布局结果等比缩放并居中，铺满画布的安全边距。
fn spread_to_canvas(points: &mut [(f64, f64)]) {
    if points.is_empty() {
        return;
    }
    let (mut min_x, mut max_x) = (f64::MAX, f64::MIN);
    let (mut min_y, mut max_y) = (f64::MAX, f64::MIN);
    for (x, y) in points.iter() {
        min_x = min_x.min(*x);
        max_x = max_x.max(*x);
        min_y = min_y.min(*y);
        max_y = max_y.max(*y);
    }
    let width = (max_x - min_x).max(1.0);
    let height = (max_y - min_y).max(1.0);
    let padding = 90.0;
    // 上界避免一两个节点被放大到失真，下界避免大图被压成一团。
    let scale = ((SCENE_WIDTH - padding * 2.0) / width)
        .min((SCENE_HEIGHT - padding * 2.0) / height)
        .clamp(0.2, 3.0);
    let center_x = (min_x + max_x) / 2.0;
    let center_y = (min_y + max_y) / 2.0;
    for (x, y) in points.iter_mut() {
        *x = (SCENE_WIDTH / 2.0 + (*x - center_x) * scale).clamp(60.0, SCENE_WIDTH - 60.0);
        *y = (SCENE_HEIGHT / 2.0 + (*y - center_y) * scale).clamp(60.0, SCENE_HEIGHT - 60.0);
    }
}

fn related_ids(edges: &[MemoryEdge], id: &str) -> BTreeSet<String> {
    edges
        .iter()
        .filter_map(|edge| {
            if edge.source == id {
                Some(edge.target.clone())
            } else if edge.target == id {
                Some(edge.source.clone())
            } else {
                None
            }
        })
        .collect()
}

fn degree_of(edges: &[MemoryEdge], id: &str) -> usize {
    edges
        .iter()
        .filter(|edge| edge.source == id || edge.target == id)
        .count()
}

fn radius(degree: usize) -> f64 {
    11.0 + (degree.min(6) as f64) * 1.8
}

fn kind_key(node: &MemoryNode) -> &'static str {
    match node.kind {
        az_memory_model::NodeKind::Note => "note",
        az_memory_model::NodeKind::Concept => "concept",
        az_memory_model::NodeKind::Person => "person",
        az_memory_model::NodeKind::Event => "event",
        az_memory_model::NodeKind::Project => "project",
        az_memory_model::NodeKind::Source => "source",
    }
}

fn truncate(title: &str) -> String {
    let text: String = title.chars().take(14).collect();
    if title.chars().count() > 14 {
        format!("{text}…")
    } else {
        text
    }
}

fn tag_text(tags: &[String]) -> String {
    tags.iter()
        .map(|tag| format!("#{tag}"))
        .collect::<Vec<_>>()
        .join(" ")
}
