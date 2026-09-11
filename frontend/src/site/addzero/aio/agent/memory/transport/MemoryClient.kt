@file:OptIn(kotlin.js.ExperimentalWasmJsInterop::class)

package site.addzero.aio.agent.memory.transport

import kotlinx.coroutines.await
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import site.addzero.aio.agent.memory.model.*
import kotlin.js.*

private fun invoke(method: JsString, path: JsString, payload: JsString): Promise<JsString> =
    js("window.aioPlugin.json(method, path, payload ? JSON.parse(payload) : undefined).then(value => JSON.stringify(value))")

internal object MemoryClient {
    private suspend inline fun <reified T> read(method: String, path: String, payload: String = ""): T =
        Json.decodeFromString(invoke(method.toJsString(), path.toJsString(), payload.toJsString()).await<JsString>().toString())
    suspend fun graph(query: String = ""): MemoryGraph = if (query.isBlank()) read("GET", "/graph") else
        read("POST", "/search", Json.encodeToString(SearchRequest(query)))
    suspend fun node(id: String): MemoryNode = read("GET", "/nodes/$id")
    suspend fun edges(id: String): List<MemoryEdge> = read("GET", "/nodes/$id/edges")
    suspend fun save(value: NodeDraft, id: String?): MemoryNode = read(if (id == null) "POST" else "PUT", if (id == null) "/nodes" else "/nodes/$id", Json.encodeToString(value))
    suspend fun link(value: EdgeDraft): MemoryEdge = read("POST", "/edges", Json.encodeToString(value))
    suspend fun remove(path: String) { invoke("DELETE".toJsString(), path.toJsString(), "".toJsString()).await<JsString>() }
    suspend fun import(value: ImportRequest): ImportResult = read("POST", "/import", Json.encodeToString(value))
    suspend fun context(value: ContextRequest): ContextResult = read("POST", "/context", Json.encodeToString(value))
}
