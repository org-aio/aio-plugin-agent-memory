package site.addzero.aio.agent.memory.workspace

import androidx.compose.runtime.*
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import kotlinx.serialization.json.Json
import site.addzero.aio.agent.memory.access.*
import site.addzero.aio.agent.memory.intake.*
import site.addzero.aio.agent.memory.model.*
import site.addzero.aio.agent.memory.navigation.MemoryRoute
import site.addzero.aio.agent.memory.navigation.RouteBridge
import site.addzero.aio.agent.memory.transport.MemoryClient

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
    var view by mutableStateOf("wiki")
    var spaces by mutableStateOf<List<MemorySpace>>(emptyList())
    var space by mutableStateOf<MemorySpace?>(null)
    var sources by mutableStateOf<List<SourceView>>(emptyList())
    var secrets by mutableStateOf<List<SecretSummary>>(emptyList())
    var animate by mutableStateOf(true)
    var zoom by mutableStateOf(1f)
    var focused by mutableStateOf(false)
    private var selection = 0
    private var serverQuery = ""
    private var restoring = false

    // 写回 URL 时派发的 hashchange 会被自己的订阅收到；用它抑制回环。
    private var writing = false

    // 链接里指定的空间与节点；数据加载完成后落地。
    private var routeSpaceId: String? = null
    private var routeNodeId: String? = null

    init {
        // 首屏从链接还原状态，之后跟随宿主片段变化（后退、粘贴新链接）。
        applyRoute(MemoryRoute.parse(RouteBridge.read()))
        RouteBridge.subscribe {
            if (!restoring && !writing) applyRoute(MemoryRoute.parse(RouteBridge.read()))
        }
    }

    /** 把当前界面状态写回 URL 片段；同一状态只对应一条规范片段。 */
    private fun publish() {
        if (restoring) return
        writing = true
        try {
            RouteBridge.write(MemoryRoute(view, selected?.id, space?.id, query, kind).encode())
        } finally {
            writing = false
        }
    }

    /** 从链接恢复状态；缺失或非法字段已由 parse 回落到默认值。 */
    private fun applyRoute(route: MemoryRoute) {
        restoring = true
        try {
            view = route.view
            query = route.query
            kind = route.kind
            routeSpaceId = route.spaceId
            routeNodeId = route.nodeId
            selected = null
        } finally {
            restoring = false
        }
    }

    /** 数据就绪后落地链接指定的空间与节点。 */
    private fun restoreRoute() {
        routeSpaceId?.let { id ->
            routeSpaceId = null
            spaces.firstOrNull { it.id == id }?.let { match ->
                if (match.id != space?.id) switchSpace(match, fromRoute = true)
            }
        }
        routeNodeId?.let { id ->
            routeNodeId = null
            graph.nodes.firstOrNull { it.id == id }?.let(::select)
        }
    }

    val visible: List<MemoryNode>
        get() {
            val matches =
                graph.nodes.filter {
                    (kind == null || kind == it.kind) &&
                        (query.isBlank() ||
                            it.title.contains(query, true) ||
                            it.content.contains(query, true) ||
                            (it.tags + it.aliases).any { tag -> tag.contains(query, true) } ||
                            query == serverQuery)
                }
            val ids =
                if (focused && selected != null)
                    neighborhood(setOf(selected!!.id), graph.edges, 1, 200)
                else null
            return matches.filter { ids == null || it.id in ids }
        }

    fun run(action: suspend () -> Unit) {
        if (busy) return
        busy = true
        error = null
        scope.launch {
            try {
                action()
            } catch (cause: CancellationException) {
                throw cause
            } catch (cause: Throwable) {
                report(cause)
            } finally {
                busy = false
            }
        }
    }

    private fun report(cause: Throwable) {
        val message = cause.message ?: "请求失败"
        error =
            runCatching { Json.decodeFromString<Failure>(message).error }
                .getOrDefault(message.take(240))
    }

    fun refresh(search: Boolean = false) = run {
        spaces = MemoryClient.spaces()
        space = spaces.firstOrNull { it.id == space?.id } ?: spaces.firstOrNull()
        MemoryClient.spaceId = space?.id
        if (search) serverQuery = query
        graph = MemoryClient.graph(serverQuery)
        sources = MemoryClient.sources().sources
        secrets = MemoryClient.secrets()
        selected?.let { previous ->
            graph.nodes.firstOrNull { it.id == previous.id }?.let(::select) ?: closeDetail()
        }
        restoreRoute()
        publish()
    }

    fun switchSpace(value: MemorySpace, fromRoute: Boolean = false) {
        if (busy) return
        closeDetail()
        dialog = null
        context = null
        graph = MemoryGraph(emptyList(), emptyList(), 0)
        sources = emptyList()
        secrets = emptyList()
        if (!fromRoute) {
            query = ""
            serverQuery = ""
        }
        space = value
        MemoryClient.spaceId = value.id
        refresh()
    }

    fun openSource(id: String) = run { dialog = MemoryDialog.Source(MemoryClient.source(id)) }

    fun retry(source: SourceView) = run {
        dialog = MemoryDialog.Source(MemoryClient.retry(source.id))
        sources = MemoryClient.sources().sources
    }

    fun review(source: SourceView) = run {
        val proposal =
            MemoryClient.proposal(source.id) ?: throw IllegalStateException("待整理记录已改变，请刷新")
        val current =
            proposal.entries.mapNotNull { it.existingId }.distinct().map { MemoryClient.node(it) }
        dialog = MemoryDialog.Review(source, proposal, current)
    }

    fun resolve(review: MemoryDialog.Review, accept: Boolean) = run {
        dialog =
            MemoryDialog.Source(
                MemoryClient.resolve(
                    review.source.id,
                    ReviewRequest(accept, review.current.associate { it.id to it.version }),
                )
            )
        sources = MemoryClient.sources().sources
        graph = MemoryClient.graph(serverQuery)
    }

    fun history(node: MemoryNode) = run {
        dialog = MemoryDialog.History(node, MemoryClient.revisions(node.id))
    }

    fun rollback(node: MemoryNode, version: Long) = run {
        select(MemoryClient.rollback(node.id, RollbackRequest(version, node.version)))
        dialog = null
        graph = MemoryClient.graph(serverQuery)
    }

    fun clearSearch() {
        query = ""
        if (serverQuery.isNotEmpty()) refresh(search = true) else publish()
    }

    fun setView(value: String) {
        view = value
        publish()
    }

    fun setQuery(value: String) {
        query = value
        publish()
    }

    fun setKind(value: NodeKind?) {
        kind = value
        publish()
    }

    fun select(node: MemoryNode) {
        selected = node
        publish()
        val generation = ++selection
        loadingDetail = true
        connections = graph.edges.filter { it.source == node.id || it.target == node.id }
        scope.launch {
            try {
                val full = MemoryClient.node(node.id)
                val edges = MemoryClient.edges(node.id)
                if (generation == selection) {
                    selected = full
                    connections = edges
                }
            } catch (cause: CancellationException) {
                throw cause
            } catch (cause: Throwable) {
                if (generation == selection) report(cause)
            } finally {
                if (generation == selection) loadingDetail = false
            }
        }
    }

    fun closeDetail() {
        selection++
        selected = null
        loadingDetail = false
        focused = false
        publish()
    }

    fun follow(id: String) {
        graph.nodes
            .firstOrNull { it.id == id }
            ?.let {
                select(it)
                return
            }
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
    data class Source(val value: SourceView) : MemoryDialog

    data class History(val node: MemoryNode, val versions: List<WikiRevision>) : MemoryDialog

    data class Review(
        val source: SourceView,
        val proposal: CompilationResult,
        val current: List<MemoryNode>,
    ) : MemoryDialog

    data class Node(val existing: MemoryNode? = null) : MemoryDialog

    data class Edge(val source: String) : MemoryDialog

    data class Delete(val id: String, val title: String, val edge: Boolean = false) : MemoryDialog

    data object Import : MemoryDialog

    data object Context : MemoryDialog
}
