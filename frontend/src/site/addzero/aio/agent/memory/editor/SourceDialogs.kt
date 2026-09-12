@file:OptIn(kotlin.js.ExperimentalWasmJsInterop::class)

package site.addzero.aio.agent.memory.editor

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import kotlin.js.*
import kotlinx.coroutines.await
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*
import site.addzero.aio.agent.memory.model.ImportRequest
import site.addzero.aio.agent.memory.workspace.*

private fun chooseFile(): Promise<JsString> =
    js(
        """new Promise((resolve, reject) => {
  const input = document.createElement('input'); input.type = 'file'; input.accept = '.md,.markdown,.txt';
  input.oncancel = () => resolve('null');
  input.onchange = async () => { try {
    const file = input.files[0]; if (!file) return resolve('null');
    if (file.size > 90000) throw new Error('文件不能超过 90 KB');
    resolve(JSON.stringify({name: file.name, text: await file.text()}));
  } catch (error) { reject(error); } }; input.click();
})"""
    )

private fun requestId(): JsString = js("crypto.randomUUID()")

@Composable
internal fun ImportEditor(state: MemoryState) {
    var title by remember { mutableStateOf("") }
    var text by remember { mutableStateOf("") }
    var url by remember { mutableStateOf("") }
    val request = remember(title, text, url) { requestId().toString() }
    val scope = rememberCoroutineScope()
    AlertDialog(
        onDismissRequest = { if (!state.busy) state.dialog = null },
        title = { Text("导入来源") },
        text = {
            Column(
                Modifier.heightIn(max = 520.dp).verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                OutlinedButton(
                    onClick = {
                        scope.launch {
                            try {
                                val result =
                                    Json.parseToJsonElement(
                                        chooseFile().await<JsString>().toString()
                                    )
                                if (result != JsonNull) {
                                    title =
                                        result.jsonObject
                                            .getValue("name")
                                            .jsonPrimitive
                                            .content
                                            .substringBeforeLast('.')
                                    text = result.jsonObject.getValue("text").jsonPrimitive.content
                                }
                            } catch (cause: Throwable) {
                                state.error = cause.message
                            }
                        }
                    },
                    enabled = !state.busy,
                ) {
                    Text("选择 Markdown / 文本文件")
                }
                OutlinedTextField(
                    title,
                    { title = it },
                    label = { Text("来源标题") },
                    singleLine = true,
                    enabled = !state.busy,
                )
                OutlinedTextField(
                    url,
                    { url = it },
                    label = { Text("来源 URL") },
                    singleLine = true,
                    enabled = !state.busy,
                )
                OutlinedTextField(
                    text,
                    { text = it },
                    label = { Text("正文 · [[双向链接]]") },
                    minLines = 6,
                    maxLines = 12,
                    enabled = !state.busy,
                )
                DialogError(state)
            }
        },
        confirmButton = {
            TextButton(
                onClick = { state.import(ImportRequest(request, title, text, url)) },
                enabled = title.isNotBlank() && text.isNotBlank() && !state.busy,
            ) {
                Text("导入")
            }
        },
        dismissButton = {
            TextButton(onClick = { state.dialog = null }, enabled = !state.busy) { Text("取消") }
        },
    )
}

@Composable
internal fun ContextViewer(state: MemoryState) {
    val value = state.context ?: return
    AlertDialog(
        onDismissRequest = { state.dialog = null },
        title = { Text("上下文 · ${value.nodeIds.size} 节点") },
        text = {
            Column {
                OutlinedTextField(
                    value.markdown,
                    {},
                    readOnly = true,
                    modifier = Modifier.fillMaxWidth().heightIn(max = 440.dp),
                    minLines = 8,
                    maxLines = 18,
                )
                if (value.truncated) Text("上下文已截断", color = MaterialTheme.colorScheme.error)
            }
        },
        confirmButton = { TextButton(onClick = { state.dialog = null }) { Text("关闭") } },
    )
}
