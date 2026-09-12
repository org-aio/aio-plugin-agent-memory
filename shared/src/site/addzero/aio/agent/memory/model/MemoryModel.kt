package site.addzero.aio.agent.memory.model

import kotlinx.serialization.Serializable

@Serializable
enum class NodeKind(val label: String) {
    NOTE("笔记"),
    CONCEPT("概念"),
    PERSON("人物"),
    EVENT("事件"),
    SOURCE("来源"),
    PROJECT("项目"),
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
    val aliases: List<String> = emptyList(),
)

@Serializable
data class NodeDraft(
    val title: String,
    val kind: NodeKind = NodeKind.NOTE,
    val content: String = "",
    val url: String = "",
    val tags: List<String> = emptyList(),
    val version: Long? = null,
    val aliases: List<String> = emptyList(),
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
data class EdgeDraft(
    val source: String,
    val target: String,
    val relation: String,
    val evidence: String = "",
)

@Serializable
data class MemoryGraph(
    val nodes: List<MemoryNode>,
    val edges: List<MemoryEdge>,
    val total: Long,
    val truncated: Boolean = false,
)

@Serializable
data class SearchRequest(val query: String = "", val kind: NodeKind? = null, val limit: Int = 200)

@Serializable data class RecallRequest(val query: String, val limit: Int = 8)

@Serializable data class VisibilityRequest(val nodeIds: List<String>)

@Serializable
data class ImportRequest(
    val requestId: String,
    val title: String,
    val text: String,
    val url: String = "",
)

@Serializable data class ImportResult(val source: MemoryNode, val linkedNodes: Int)

@Serializable
data class ContextRequest(
    val nodeIds: List<String>,
    val depth: Int = 1,
    val maxCharacters: Int = 16000,
)

@Serializable
data class ContextResult(val markdown: String, val nodeIds: List<String>, val truncated: Boolean)

@Serializable data class Failure(val error: String)

@Serializable
data class WikiRevision(
    val version: Long,
    val draft: NodeDraft,
    val authorId: String,
    val authorType: String,
    val sourceId: String?,
    val updatedAt: Long,
)

@Serializable data class RollbackRequest(val version: Long, val currentVersion: Long)

class InputFailure(message: String) : IllegalArgumentException(message)

class MissingRecord : IllegalStateException("记录不存在或已被删除")

class StaleRecord : IllegalStateException("内容已被其他会话修改，请刷新后重试")
