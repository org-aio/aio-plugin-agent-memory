package site.addzero.aio.agent.memory.workspace

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import site.addzero.aio.agent.memory.intake.sourceTitle

internal fun statusLabel(status: String) =
    when (status) {
        "pending" -> "已接收"
        "processing" -> "整理中"
        "complete" -> "已整理"
        "quarantined" -> "保密待处理"
        "conflict" -> "待核实"
        "failed" -> "整理失败"
        else -> "待处理"
    }

@Composable
internal fun SpaceSelector(state: MemoryState) {
    var expanded by remember { mutableStateOf(false) }
    Box {
        TextButton(onClick = { expanded = true }, enabled = !state.busy) {
            Icon(Icons.Default.Folder, null)
            Text(state.space?.title ?: "个人记忆", maxLines = 1, overflow = TextOverflow.Ellipsis)
            Icon(Icons.Default.ArrowDropDown, null)
        }
        DropdownMenu(expanded, { expanded = false }) {
            state.spaces.forEach { space ->
                DropdownMenuItem(
                    text = { Text(space.title) },
                    onClick = {
                        expanded = false
                        state.switchSpace(space)
                    },
                )
            }
        }
    }
}

@Composable
internal fun ViewSelector(state: MemoryState) {
    val views =
        linkedMapOf(
            "wiki" to "Wiki",
            "graph" to "图谱",
            "sources" to "来源",
            "secrets" to "凭据",
            "pending" to "待整理",
        )
    var expanded by remember { mutableStateOf(false) }
    Box {
        TextButton(onClick = { expanded = true }) {
            Text(views[state.view] ?: "Wiki")
            Icon(Icons.Default.ArrowDropDown, null)
        }
        DropdownMenu(expanded, { expanded = false }) {
            views.forEach { (id, label) ->
                DropdownMenuItem(
                    text = { Text(label) },
                    onClick = {
                        state.view = id
                        expanded = false
                    },
                )
            }
        }
    }
}

@Composable
internal fun IntakeList(state: MemoryState, modifier: Modifier) {
    LazyColumn(modifier.fillMaxWidth()) {
        if (state.view == "secrets") {
            if (state.secrets.isEmpty()) item { Text("暂无凭据", Modifier.padding(24.dp)) }
            items(state.secrets, key = { it.id }) { secret ->
                ListItem(
                    headlineContent = { Text(secret.label) },
                    supportingContent = { Text("********") },
                    leadingContent = { Icon(Icons.Default.Lock, null) },
                    trailingContent = {
                        Icon(
                            if (secret.canReveal) Icons.Default.Visibility else Icons.Default.Lock,
                            null,
                        )
                    },
                    modifier = Modifier.clickable { state.openSource(secret.sourceId) },
                )
                HorizontalDivider()
            }
        } else {
            val sources =
                state.sources.filter { state.view != "pending" || it.status != "complete" }
            if (sources.isEmpty()) item { Text("暂无资料", Modifier.padding(24.dp)) }
            items(sources, key = { it.id }) { source ->
                ListItem(
                    headlineContent = {
                        Text(
                            sourceTitle(source.text),
                            maxLines = 2,
                            overflow = TextOverflow.Ellipsis,
                        )
                    },
                    supportingContent = { Text(statusLabel(source.status)) },
                    trailingContent = { Icon(Icons.Default.ChevronRight, null) },
                    modifier = Modifier.clickable { state.openSource(source.id) },
                )
                HorizontalDivider()
            }
        }
    }
}
