package site.addzero.aio.agent.memory.editor

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import site.addzero.aio.agent.memory.intake.*
import site.addzero.aio.agent.memory.transport.MemoryClient
import site.addzero.aio.agent.memory.workspace.*

@Composable
internal fun ReceiptDialog(state: MemoryState, source: SourceView) {
    AlertDialog(
        onDismissRequest = { state.dialog = null },
        title = { Text(statusLabel(source.status)) },
        text = {
            Column(
                Modifier.heightIn(max = 480.dp).verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                SelectionContainer { Text(source.text) }
                source.secrets.forEach { secret -> SecretPreview(state, secret) }
                source.error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
                if (source.status in setOf("pending", "failed"))
                    TextButton(onClick = { state.retry(source) }, enabled = !state.busy) {
                        Text("重试整理")
                    }
                if (source.status == "conflict")
                    TextButton(onClick = { state.review(source) }, enabled = !state.busy) {
                        Text("查看待核实修订")
                    }
                DialogError(state)
            }
        },
        confirmButton = { TextButton(onClick = { state.dialog = null }) { Text("关闭") } },
    )
}

@Composable
internal fun ReviewDialog(state: MemoryState, review: MemoryDialog.Review) {
    AlertDialog(
        onDismissRequest = { state.dialog = null },
        title = { Text("核实修订") },
        text = {
            Column(
                Modifier.heightIn(max = 480.dp).verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                review.proposal.entries.forEach { entry ->
                    Text(entry.draft.title, style = MaterialTheme.typography.titleMedium)
                    review.current
                        .firstOrNull { it.id == entry.existingId }
                        ?.let { node ->
                            Text(
                                "当前 · v${node.version}",
                                style = MaterialTheme.typography.labelMedium,
                            )
                            SelectionContainer { Text(node.content) }
                        }
                    Text("建议修订", style = MaterialTheme.typography.labelMedium)
                    SelectionContainer { Text(entry.draft.content) }
                    HorizontalDivider()
                }
                DialogError(state)
            }
        },
        confirmButton = {
            TextButton(onClick = { state.resolve(review, true) }, enabled = !state.busy) {
                Text("接受修订")
            }
        },
        dismissButton = {
            TextButton(onClick = { state.resolve(review, false) }, enabled = !state.busy) {
                Text("保留现有内容")
            }
        },
    )
}

@Composable
private fun SecretPreview(state: MemoryState, secret: SecretSummary) {
    var value by remember(secret.id) { mutableStateOf<String?>(null) }
    DisposableEffect(secret.id) { onDispose { value = null } }
    LaunchedEffect(value) {
        if (value != null) {
            delay(30_000)
            value = null
        }
    }
    Column {
        Row {
            Text(secret.label, Modifier.weight(1f))
            Tool(
                if (value == null) "查看秘密" else "隐藏秘密",
                if (value == null) Icons.Default.Visibility else Icons.Default.VisibilityOff,
                secret.canReveal && !state.busy,
            ) {
                if (value != null) value = null
                else state.run { value = MemoryClient.reveal(secret.id).value }
            }
            Tool("复制秘密", Icons.Default.ContentCopy, value != null && !state.busy) {
                value?.let { secretValue -> state.run { MemoryClient.copy(secretValue) } }
            }
        }
        Box(Modifier.fillMaxWidth().height(96.dp).verticalScroll(rememberScrollState())) {
            SelectionContainer { Text(value ?: "********") }
        }
    }
}

@Composable
internal fun RevisionDialog(state: MemoryState, dialog: MemoryDialog.History) {
    var selected by remember { mutableStateOf<Long?>(null) }
    AlertDialog(
        onDismissRequest = { state.dialog = null },
        title = { Text("版本记录") },
        text = {
            Column(
                Modifier.heightIn(max = 480.dp).verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                dialog.versions.forEach { revision ->
                    Text(
                        "v${revision.version} · ${revision.authorId}",
                        style = MaterialTheme.typography.titleSmall,
                    )
                    SelectionContainer { Text(revision.draft.content) }
                    if (revision.version != dialog.node.version)
                        TextButton(onClick = { selected = revision.version }) { Text("回退到此版本") }
                    HorizontalDivider()
                }
                DialogError(state)
            }
        },
        confirmButton = { TextButton(onClick = { state.dialog = null }) { Text("关闭") } },
    )
    selected?.let { version ->
        AlertDialog(
            onDismissRequest = { selected = null },
            title = { Text("回退到 v$version？") },
            text = { Text(dialog.node.title) },
            confirmButton = {
                TextButton(
                    onClick = {
                        selected = null
                        state.rollback(dialog.node, version)
                    }
                ) {
                    Text("确认回退")
                }
            },
            dismissButton = { TextButton(onClick = { selected = null }) { Text("取消") } },
        )
    }
}
