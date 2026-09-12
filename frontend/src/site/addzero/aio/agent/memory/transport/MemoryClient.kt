@file:OptIn(kotlin.js.ExperimentalWasmJsInterop::class)

package site.addzero.aio.agent.memory.transport

import kotlin.js.*
import kotlinx.coroutines.await
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import site.addzero.aio.agent.memory.access.*
import site.addzero.aio.agent.memory.intake.*
import site.addzero.aio.agent.memory.model.*

private fun invoke(method: JsString, path: JsString, payload: JsString): Promise<JsString> =
    js(
        "window.aioPlugin.json(method, path, payload ? JSON.parse(payload) : undefined).then(value => JSON.stringify(value))"
    )

private fun copyText(value: JsString): Promise<JsAny?> = js("window.aioPlugin.copy(value)")

internal object MemoryClient {
    suspend fun copy(value: String) {
        copyText(value.toJsString()).await<JsAny?>()
    }

    var spaceId: String? = null

    private fun scoped(path: String) = spaceId?.let { "$path?spaceId=$it" } ?: path

    private suspend inline fun <reified T> read(
        method: String,
        path: String,
        payload: String = "",
    ): T =
        Json.decodeFromString(
            invoke(method.toJsString(), scoped(path).toJsString(), payload.toJsString())
                .await<JsString>()
                .toString()
        )

    suspend fun spaces(): List<MemorySpace> = read("GET", "/spaces")

    suspend fun sources(): SourceList = read("GET", "/sources")

    suspend fun source(id: String): SourceView = read("GET", "/sources/$id")

    suspend fun secrets(): List<SecretSummary> = read("GET", "/secrets")

    suspend fun reveal(id: String): RevealedSecret = read("POST", "/secrets/$id/reveal")

    suspend fun retry(id: String): SourceView = read("POST", "/sources/$id/retry")

    suspend fun proposal(id: String): CompilationResult? = read("GET", "/sources/$id/proposal")

    suspend fun resolve(id: String, request: ReviewRequest): SourceView =
        read("POST", "/sources/$id/resolve", Json.encodeToString(request))

    suspend fun revisions(id: String): List<WikiRevision> = read("GET", "/nodes/$id/revisions")

    suspend fun rollback(id: String, value: RollbackRequest): MemoryNode =
        read("POST", "/nodes/$id/rollback", Json.encodeToString(value))

    suspend fun graph(query: String = ""): MemoryGraph =
        if (query.isBlank()) read("GET", "/graph")
        else read("POST", "/search", Json.encodeToString(SearchRequest(query)))

    suspend fun node(id: String): MemoryNode = read("GET", "/nodes/$id")

    suspend fun edges(id: String): List<MemoryEdge> = read("GET", "/nodes/$id/edges")

    suspend fun save(value: NodeDraft, id: String?): MemoryNode =
        read(
            if (id == null) "POST" else "PUT",
            if (id == null) "/nodes" else "/nodes/$id",
            Json.encodeToString(value),
        )

    suspend fun link(value: EdgeDraft): MemoryEdge =
        read("POST", "/edges", Json.encodeToString(value))

    suspend fun remove(path: String) {
        invoke("DELETE".toJsString(), scoped(path).toJsString(), "".toJsString()).await<JsString>()
    }

    suspend fun import(value: ImportRequest): ImportResult =
        read("POST", "/import", Json.encodeToString(value))

    suspend fun context(value: ContextRequest): ContextResult =
        read("POST", "/context", Json.encodeToString(value))
}
