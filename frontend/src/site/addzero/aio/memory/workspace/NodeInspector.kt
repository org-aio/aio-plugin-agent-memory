package site.addzero.aio.memory.workspace

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp

@Composable
internal fun NodeInspector(state: MemoryState) {
    val node = state.selected
    if (node == null) {
        Box(Modifier.fillMaxSize().padding(24.dp), contentAlignment = Alignment.Center) {
            Text("未选择节点", color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        return
    }
    Column(Modifier.fillMaxSize().padding(16.dp).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(node.kind.label, Modifier.weight(1f), color = node.kind.color(), style = MaterialTheme.typography.labelLarge)
            Tool("编辑记忆", Icons.Default.Edit, !state.busy && !state.loadingDetail) { state.dialog = MemoryDialog.Node(node) }
            Tool("删除记忆", Icons.Default.Delete, !state.busy && !state.loadingDetail) { state.dialog = MemoryDialog.Delete(node.id, node.title) }
            Tool("关闭详情", Icons.Default.Close) { state.closeDetail() }
        }
        Text(node.title, style = MaterialTheme.typography.titleLarge)
        if (state.loadingDetail) LinearProgressIndicator(Modifier.fillMaxWidth())
        SelectionContainer { Text(node.content.ifBlank { "暂无正文" }, style = MaterialTheme.typography.bodyMedium) }
        if (node.tags.isNotEmpty()) Text(node.tags.joinToString("  ") { "#$it" }, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.primary)
        if (node.url.isNotEmpty()) {
            Text("来源", style = MaterialTheme.typography.labelLarge)
            SelectionContainer { Text(node.url, style = MaterialTheme.typography.bodySmall) }
        }
        HorizontalDivider()
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text("关联 · ${state.connections.size}", Modifier.weight(1f), style = MaterialTheme.typography.titleSmall)
            Tool("添加关系", Icons.Default.Add, !state.busy && state.graph.nodes.size > 1) { state.dialog = MemoryDialog.Edge(node.id) }
        }
        state.connections.forEach { edge ->
            val outgoing = edge.source == node.id
            val other = if (outgoing) edge.target else edge.source
            val title = state.graph.nodes.firstOrNull { it.id == other }?.title ?: other.take(12)
            Row(verticalAlignment = Alignment.CenterVertically) {
                TextButton(onClick = { state.follow(other) }, modifier = Modifier.weight(1f)) {
                    Column(Modifier.fillMaxWidth()) {
                        Text(if (outgoing) "${edge.relation} →" else "← ${edge.relation}", style = MaterialTheme.typography.labelSmall)
                        Text(title, maxLines = 2, overflow = TextOverflow.Ellipsis)
                        if (edge.evidence.isNotBlank()) Text(edge.evidence, style = MaterialTheme.typography.bodySmall, maxLines = 2, overflow = TextOverflow.Ellipsis)
                    }
                }
                Tool("删除关系 $title", Icons.Default.Delete, !state.busy) { state.dialog = MemoryDialog.Delete(edge.id, "${edge.relation}: $title", edge = true) }
            }
        }
        if (state.connections.isEmpty()) Text("暂无关系", color = MaterialTheme.colorScheme.onSurfaceVariant)
        OutlinedButton(onClick = { state.focused = !state.focused; if (state.focused) { state.query = ""; state.kind = null } }) { Text(if (state.focused) "显示完整图谱" else "查看一层邻域") }
        OutlinedButton(onClick = { state.export() }, enabled = !state.busy) { Text("导出上下文") }
        SelectionContainer { Text("v${node.version} · ${node.id}", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
    }
}
