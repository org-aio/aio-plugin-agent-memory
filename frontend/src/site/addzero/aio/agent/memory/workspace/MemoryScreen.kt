package site.addzero.aio.agent.memory.workspace

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import site.addzero.aio.agent.memory.model.*
import site.addzero.aio.agent.memory.editor.MemoryDialogs
import site.addzero.component.knowledge_graph.main_logic.KnowledgeGraph
import site.addzero.component.knowledge_graph.model.GraphData
import site.addzero.component.knowledge_graph.model.GraphEdge
import site.addzero.component.knowledge_graph.model.GraphNode
import site.addzero.component.knowledge_graph.model.NodeCategory

internal fun NodeKind.color() = when (this) {
    NodeKind.NOTE -> Color(0xFF36776B)
    NodeKind.CONCEPT -> Color(0xFF476FBD)
    NodeKind.PERSON -> Color(0xFF9D487B)
    NodeKind.EVENT -> Color(0xFF9C7022)
    NodeKind.SOURCE -> Color(0xFF686C75)
    NodeKind.PROJECT -> Color(0xFF488349)
}

@Composable
internal fun MemoryScreen() {
    val scope = rememberCoroutineScope()
    val state = remember(scope) { MemoryState(scope) }
    LaunchedEffect(state) { state.refresh() }
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val compact = maxWidth < 760.dp
        Column(Modifier.fillMaxSize()) {
            Header(state, compact)
            HorizontalDivider()
            if (state.busy) LinearProgressIndicator(Modifier.fillMaxWidth().height(2.dp)) else Spacer(Modifier.height(2.dp))
            state.error?.let { message ->
                Row(Modifier.fillMaxWidth().background(MaterialTheme.colorScheme.errorContainer).padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(message, Modifier.weight(1f), color = MaterialTheme.colorScheme.onErrorContainer, maxLines = 3)
                    Tool("关闭错误", Icons.Default.Close) { state.error = null }
                }
            }
            Row(Modifier.weight(1f)) {
                Column(Modifier.weight(1f).fillMaxHeight()) {
                    GraphToolbar(state, compact)
                    val visible = state.visible
                    if (state.graph.nodes.isEmpty() && !state.busy) {
                        Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
                            Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(12.dp)) {
                                Icon(Icons.Default.Share, null, Modifier.size(40.dp), tint = MaterialTheme.colorScheme.primary)
                                Text(if (state.graph.total == 0L) "还没有记忆" else "没有匹配的节点", style = MaterialTheme.typography.titleMedium)
                                Button(onClick = { state.dialog = MemoryDialog.Node() }) { Icon(Icons.Default.Add, null); Text("新建记忆") }
                                TextButton(onClick = { state.dialog = MemoryDialog.Import }) { Text("导入来源") }
                            }
                        }
                    } else if (state.listMode) NodeList(state, visible, Modifier.weight(1f)) else {
                        val graph = remember(visible, state.graph.edges) {
                            val ids = visible.map { it.id }.toSet()
                            GraphData(visible.map { GraphNode(it.id, it.title, NodeCategory.DEFAULT, it.url, it.content, it.kind.label) },
                                state.graph.edges.filter { it.source in ids && it.target in ids }.map { GraphEdge(it.source, it.target, it.relation) })
                        }
                        Box(Modifier.weight(1f).fillMaxWidth()) {
                            KnowledgeGraph(graph, onNodeClick = { selected -> visible.firstOrNull { it.id == selected.id }?.let(state::select) },
                                edgeLabelProvider = { edge -> if (state.selected?.id == edge.source || state.selected?.id == edge.target) edge.label.orEmpty() else "" },
                                selectedNodeId = state.selected?.id, nodeColors = visible.associate { it.id to it.kind.color() },
                                animate = state.animate, zoom = state.zoom)
                            if (visible.isEmpty()) Text("没有匹配的节点", Modifier.align(Alignment.Center))
                        }
                    }
                    Footer(state, visible.size)
                }
                if (!compact) {
                    VerticalDivider()
                    Box(Modifier.width(320.dp).fillMaxHeight()) { NodeInspector(state) }
                }
            }
        }
        if (compact && state.selected != null && state.dialog == null) {
            androidx.compose.ui.window.Dialog(onDismissRequest = state::closeDetail) {
                Surface(shape = MaterialTheme.shapes.medium, modifier = Modifier.fillMaxWidth().heightIn(max = maxHeight - 48.dp)) { NodeInspector(state) }
            }
        }
        MemoryDialogs(state)
    }
}

@Composable
private fun Header(state: MemoryState, compact: Boolean) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 10.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Default.Share, null, Modifier.size(22.dp), tint = MaterialTheme.colorScheme.primary)
            Spacer(Modifier.width(10.dp))
            Text("记忆图谱", style = MaterialTheme.typography.titleLarge)
            Spacer(Modifier.weight(1f))
            Tool("导入来源", Icons.Default.UploadFile, !state.busy) { state.dialog = MemoryDialog.Import }
            Tool("导出上下文", Icons.Default.FileDownload, !state.busy && state.graph.nodes.isNotEmpty()) { state.export() }
            if (compact) Tool("新建记忆", Icons.Default.Add, !state.busy) { state.dialog = MemoryDialog.Node() }
            else Button(onClick = { state.dialog = MemoryDialog.Node() }, enabled = !state.busy) { Icon(Icons.Default.Add, null); Text("新建记忆") }
        }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedTextField(state.query, { state.query = it }, Modifier.weight(1f), singleLine = true,
                placeholder = { Text("搜索记忆") }, leadingIcon = { Icon(Icons.Default.Search, null) },
                trailingIcon = { if (state.query.isNotEmpty()) Tool("清空搜索", Icons.Default.Close, !state.busy) { state.clearSearch() } })
            Tool("搜索全部记忆", Icons.Default.Search, !state.busy) { state.refresh(search = true) }
            Tool("刷新", Icons.Default.Refresh, !state.busy) { state.refresh() }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun GraphToolbar(state: MemoryState, compact: Boolean) {
    var menu by remember { mutableStateOf(false) }
    Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        SingleChoiceSegmentedButtonRow {
            SegmentedButton(selected = !state.listMode, onClick = { state.listMode = false }, shape = SegmentedButtonDefaults.itemShape(0, 2)) { Text("图谱") }
            SegmentedButton(selected = state.listMode, onClick = { state.listMode = true }, shape = SegmentedButtonDefaults.itemShape(1, 2)) { Text("列表") }
        }
        Box {
            TextButton(onClick = { menu = true }) { Text(state.kind?.label ?: "全部类型"); Icon(Icons.Default.ArrowDropDown, null) }
            DropdownMenu(menu, { menu = false }) {
                DropdownMenuItem(text = { Text("全部类型") }, onClick = { state.kind = null; menu = false })
                NodeKind.entries.forEach { kind -> DropdownMenuItem(text = { Text(kind.label) }, leadingIcon = { Icon(Icons.Default.Star, null, tint = kind.color()) }, onClick = { state.kind = kind; menu = false }) }
            }
        }
        Spacer(Modifier.weight(1f))
        if (!compact && state.selected != null) FilterChip(state.focused, { state.focused = !state.focused }, label = { Text("一层邻域") })
        if (!state.listMode) {
            Tool(if (state.animate) "暂停布局" else "继续布局", if (state.animate) Icons.Default.Pause else Icons.Default.PlayArrow) { state.animate = !state.animate }
            if (!compact) {
                Tool("缩小", Icons.Default.ZoomOut, state.zoom > .4f) { state.zoom = (state.zoom - .2f).coerceAtLeast(.4f) }
                Tool("放大", Icons.Default.ZoomIn, state.zoom < 3f) { state.zoom = (state.zoom + .2f).coerceAtMost(3f) }
            }
        }
    }
}

@Composable
private fun NodeList(state: MemoryState, nodes: List<MemoryNode>, modifier: Modifier) {
    LazyColumn(modifier.fillMaxWidth()) {
        items(nodes, key = { it.id }) { node ->
            ListItem(headlineContent = { Text(node.title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                supportingContent = { Text(node.content.ifBlank { node.kind.label }, maxLines = 2, overflow = TextOverflow.Ellipsis) },
                leadingContent = { Icon(Icons.Default.Star, null, tint = node.kind.color()) },
                trailingContent = { Text(node.kind.label, style = MaterialTheme.typography.labelSmall) },
                modifier = Modifier.clickable { state.select(node) },
                colors = ListItemDefaults.colors(containerColor = if (state.selected?.id == node.id) MaterialTheme.colorScheme.secondaryContainer else MaterialTheme.colorScheme.surface))
            HorizontalDivider()
        }
    }
}

@Composable
private fun Footer(state: MemoryState, visible: Int) {
    HorizontalDivider()
    Row(Modifier.fillMaxWidth().padding(12.dp), horizontalArrangement = Arrangement.SpaceBetween) {
        Text("$visible / ${state.graph.total} 节点 · ${state.graph.edges.size} 关系", style = MaterialTheme.typography.labelSmall)
        Text(if (state.graph.truncated) "结果已截断" else "PostgreSQL", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun Tool(label: String, icon: ImageVector, enabled: Boolean = true, click: () -> Unit) {
    TooltipBox(positionProvider = TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above), tooltip = { PlainTooltip { Text(label) } }, state = rememberTooltipState()) {
        IconButton(onClick = click, enabled = enabled, modifier = Modifier.size(40.dp)) { Icon(icon, contentDescription = label, modifier = Modifier.size(20.dp)) }
    }
}
