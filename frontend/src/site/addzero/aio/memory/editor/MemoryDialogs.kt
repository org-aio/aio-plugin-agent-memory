package site.addzero.aio.memory.editor

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import site.addzero.aio.memory.model.*
import site.addzero.aio.memory.workspace.*

@Composable
internal fun MemoryDialogs(state: MemoryState) {
    when (val dialog = state.dialog) {
        is MemoryDialog.Node -> NodeEditor(state, dialog.existing)
        is MemoryDialog.Edge -> EdgeEditor(state, dialog.source)
        is MemoryDialog.Delete -> AlertDialog(
            onDismissRequest = { if (!state.busy) state.dialog = null },
            title = { Text(if (dialog.edge) "删除关系？" else "删除记忆及其关系？") },
            text = { Column { Text(dialog.title); DialogError(state) } },
            confirmButton = { TextButton(onClick = { state.remove(dialog) }, enabled = !state.busy) { Text("确认删除", color = MaterialTheme.colorScheme.error) } },
            dismissButton = { TextButton(onClick = { state.dialog = null }, enabled = !state.busy) { Text("取消") } },
        )
        MemoryDialog.Import -> ImportEditor(state)
        MemoryDialog.Context -> ContextViewer(state)
        null -> Unit
    }
}

@Composable
private fun NodeEditor(state: MemoryState, existing: MemoryNode?) {
    var title by remember { mutableStateOf(existing?.title ?: "") }
    var content by remember { mutableStateOf(existing?.content ?: "") }
    var url by remember { mutableStateOf(existing?.url ?: "") }
    var tags by remember { mutableStateOf(existing?.tags?.joinToString(", ") ?: "") }
    var kind by remember { mutableStateOf(existing?.kind ?: NodeKind.NOTE) }
    var typeMenu by remember { mutableStateOf(false) }
    AlertDialog(onDismissRequest = { if (!state.busy) state.dialog = null },
        title = { Text(if (existing == null) "新建记忆" else "编辑记忆") },
        text = {
            Column(Modifier.heightIn(max = 520.dp).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                OutlinedTextField(title, { title = it }, Modifier.fillMaxWidth(), label = { Text("标题") }, singleLine = true, enabled = !state.busy)
                Box {
                    OutlinedButton(onClick = { typeMenu = true }, enabled = !state.busy) { Text("类型：${kind.label}") }
                    DropdownMenu(typeMenu, { typeMenu = false }) { NodeKind.entries.forEach { value -> DropdownMenuItem(text = { Text(value.label) }, onClick = { kind = value; typeMenu = false }) } }
                }
                OutlinedTextField(content, { content = it }, Modifier.fillMaxWidth(), label = { Text("正文 · Markdown") }, minLines = 5, maxLines = 10, enabled = !state.busy)
                OutlinedTextField(url, { url = it }, Modifier.fillMaxWidth(), label = { Text("来源 URL") }, singleLine = true, enabled = !state.busy)
                OutlinedTextField(tags, { tags = it }, Modifier.fillMaxWidth(), label = { Text("标签（逗号分隔）") }, singleLine = true, enabled = !state.busy)
                DialogError(state)
            }
        },
        confirmButton = { TextButton(onClick = { state.save(NodeDraft(title, kind, content, url, tags.split(',', '，').map(String::trim).filter(String::isNotEmpty), existing?.version), existing?.id) }, enabled = title.isNotBlank() && !state.busy) { Text("保存") } },
        dismissButton = { TextButton(onClick = { state.dialog = null }, enabled = !state.busy) { Text("取消") } },
    )
}

@Composable
private fun EdgeEditor(state: MemoryState, source: String) {
    var target by remember { mutableStateOf<MemoryNode?>(null) }
    var search by remember { mutableStateOf("") }
    var relation by remember { mutableStateOf("相关") }
    var evidence by remember { mutableStateOf("") }
    var expanded by remember { mutableStateOf(false) }
    AlertDialog(onDismissRequest = { if (!state.busy) state.dialog = null }, title = { Text("添加关系") },
        text = {
            Column(Modifier.heightIn(max = 480.dp).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Text(state.graph.nodes.firstOrNull { it.id == source }?.title ?: source)
                OutlinedTextField(search, { search = it; target = null }, label = { Text("目标标题筛选") }, singleLine = true, enabled = !state.busy)
                Box {
                    OutlinedButton(onClick = { expanded = true }, enabled = !state.busy) { Text(target?.title ?: "选择目标节点") }
                    DropdownMenu(expanded, { expanded = false }, Modifier.heightIn(max = 240.dp)) {
                        state.graph.nodes.filter { it.id != source && it.title.contains(search, true) }.forEach { node ->
                            DropdownMenuItem(text = { Text(node.title, maxLines = 2) }, onClick = { target = node; expanded = false })
                        }
                    }
                }
                OutlinedTextField(relation, { relation = it }, label = { Text("关系名称") }, singleLine = true, enabled = !state.busy)
                OutlinedTextField(evidence, { evidence = it }, label = { Text("关系依据") }, minLines = 2, maxLines = 5, enabled = !state.busy)
                DialogError(state)
            }
        },
        confirmButton = { TextButton(onClick = { target?.let { state.link(EdgeDraft(source, it.id, relation, evidence)) } }, enabled = target != null && relation.isNotBlank() && !state.busy) { Text("保存关系") } },
        dismissButton = { TextButton(onClick = { state.dialog = null }, enabled = !state.busy) { Text("取消") } },
    )
}

@Composable
internal fun DialogError(state: MemoryState) {
    state.error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
}
