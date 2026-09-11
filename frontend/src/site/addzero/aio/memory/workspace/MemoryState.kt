package site.addzero.aio.memory.workspace

import androidx.compose.runtime.*
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import kotlinx.serialization.json.Json
import site.addzero.aio.memory.model.*
import site.addzero.aio.memory.transport.MemoryClient

internal class MemoryState(private val scope: CoroutineScope) {
    var graph by mutableStateOf(MemoryGraph(emptyList(), emptyList(), 0))
    var query by mutableStateOf("")
    var kind by mutableStateOf<NodeKind?>(null)
    var selected by mutableStateOf<MemoryNode?>(null)
    var connections by mutableStateOf<List<MemoryEdge>>(emptyList())
    var busy by mutableStateOf(false)
    var loadingDetail by mutableStateOf(false)
    var error by mutableStateOf<String?>(null)
    var dialog by mutableStateOf<MemoryDialog?>(null)
    var context by mutableStateOf<ContextResult?>(null)
    var listMode by mutableStateOf(false)
    var animate by mutableStateOf(true)
    var zoom by mutableStateOf(1f)
    var focused by mutableStateOf(false)
    private var selection = 0
    private var serverQuery = ""

    val visible: List<MemoryNode> get() {
        val matches = graph.nodes.filter {
            (kind == null || kind == it.kind) &&
                (query.isBlank() || it.title.contains(query, true) || it.content.contains(query, true) || it.tags.any { tag -> tag.contains(query, true) } || query == serverQuery)
        }
        val ids = if (focused && selected != null) neighborhood(setOf(selected!!.id), graph.edges, 1, 200) else null
        return matches.filter { ids == null || it.id in ids }
    }

    fun run(action: suspend () -> Unit) {
        if (busy) return
        busy = true
        error = null
        scope.launch {
            try { action() }
            catch (cause: CancellationException) { throw cause }
            catch (cause: Throwable) { report(cause) }
            finally { busy = false }
        }
    }
    private fun report(cause: Throwable) {
        val message = cause.message ?: "请求失败"
        error = runCatching { Json.decodeFromString<Failure>(message).error }.getOrDefault(message.take(240))
    }
    fun refresh(search: Boolean = false) = run {
        if (search) serverQuery = query
        graph = MemoryClient.graph(serverQuery)
        selected?.let { previous ->
            graph.nodes.firstOrNull { it.id == previous.id }?.let(::select) ?: closeDetail()
        }
    }
    fun clearSearch() {
        query = ""
        if (serverQuery.isNotEmpty()) refresh(search = true)
    }
    fun select(node: MemoryNode) {
        selected = node
        val generation = ++selection
        loadingDetail = true
        connections = graph.edges.filter { it.source == node.id || it.target == node.id }
        scope.launch {
            try {
                val full = MemoryClient.node(node.id)
                val edges = MemoryClient.edges(node.id)
                if (generation == selection) { selected = full; connections = edges }
            } catch (cause: CancellationException) { throw cause }
            catch (cause: Throwable) { if (generation == selection) report(cause) }
            finally { if (generation == selection) loadingDetail = false }
        }
    }
    fun closeDetail() { selection++; selected = null; loadingDetail = false; focused = false }
    fun follow(id: String) {
        graph.nodes.firstOrNull { it.id == id }?.let { select(it); return }
        run { select(MemoryClient.node(id)) }
    }
    fun save(draft: NodeDraft, id: String?) = run {
        val node = MemoryClient.save(draft.validated(), id)
        graph = MemoryClient.graph(serverQuery)
        dialog = null
        select(node)
    }
    fun link(draft: EdgeDraft) = run {
        MemoryClient.link(draft.validated())
        graph = MemoryClient.graph(serverQuery)
        dialog = null
        selected?.let(::select)
    }
    fun remove(target: MemoryDialog.Delete) = run {
        MemoryClient.remove("/${if (target.edge) "edges" else "nodes"}/${target.id}")
        graph = MemoryClient.graph(serverQuery)
        dialog = null
        if (!target.edge && selected?.id == target.id) closeDetail() else selected?.let(::select)
    }
    fun import(value: ImportRequest) = run {
        val result = MemoryClient.import(value)
        serverQuery = ""
        query = ""
        kind = null
        graph = MemoryClient.graph()
        dialog = null
        select(result.source)
    }
    fun export(depth: Int = 1) = run {
        val ids = selected?.let { listOf(it.id) } ?: visible.take(12).map { it.id }
        context = MemoryClient.context(ContextRequest(ids, depth))
        dialog = MemoryDialog.Context
    }
}

internal sealed interface MemoryDialog {
    data class Node(val existing: MemoryNode? = null) : MemoryDialog
    data class Edge(val source: String) : MemoryDialog
    data class Delete(val id: String, val title: String, val edge: Boolean = false) : MemoryDialog
    data object Import : MemoryDialog
    data object Context : MemoryDialog
}
