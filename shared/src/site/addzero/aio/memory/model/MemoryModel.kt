package site.addzero.aio.memory.model

import kotlinx.serialization.Serializable

@Serializable
enum class NodeKind(val label: String) {
    NOTE("笔记"), CONCEPT("概念"), PERSON("人物"), EVENT("事件"), SOURCE("来源"), PROJECT("项目")
}

@Serializable
data class MemoryNode(
    val id: String,
    val title: String,
    val kind: NodeKind,
    val content: String = "",
    val url: String = "",
    val tags: List<String> = emptyList(),
    val version: Long = 1,
    val updatedAt: Long = 0,
)

@Serializable
data class NodeDraft(
    val title: String,
    val kind: NodeKind = NodeKind.NOTE,
    val content: String = "",
    val url: String = "",
    val tags: List<String> = emptyList(),
    val version: Long? = null,
)

@Serializable
data class MemoryEdge(
    val id: String,
    val source: String,
    val target: String,
    val relation: String,
    val evidence: String = "",
)

@Serializable
data class EdgeDraft(val source: String, val target: String, val relation: String, val evidence: String = "")

@Serializable
data class MemoryGraph(val nodes: List<MemoryNode>, val edges: List<MemoryEdge>, val total: Long, val truncated: Boolean = false)

@Serializable
data class SearchRequest(val query: String = "", val kind: NodeKind? = null, val limit: Int = 200)

@Serializable
data class ImportRequest(val title: String, val text: String, val url: String = "")

@Serializable
data class ImportResult(val source: MemoryNode, val linkedNodes: Int)

@Serializable
data class ContextRequest(val nodeIds: List<String>, val depth: Int = 1, val maxCharacters: Int = 16000)

@Serializable
data class ContextResult(val markdown: String, val nodeIds: List<String>, val truncated: Boolean)

@Serializable
data class Failure(val error: String)

class InputFailure(message: String) : IllegalArgumentException(message)
class MissingRecord : IllegalStateException("记录不存在或已被删除")
class StaleRecord : IllegalStateException("内容已被其他会话修改，请刷新后重试")
