package site.addzero.aio.agent.memory.intake

import kotlinx.serialization.Serializable
import site.addzero.aio.agent.memory.model.MemoryNode
import site.addzero.aio.agent.memory.model.NodeDraft

@Serializable
data class CaptureRequest(
    val requestId: String,
    val text: String,
    val spaceId: String? = null,
    val origin: String = "chat",
    val reference: String = "",
    val clarifies: String? = null,
)

@Serializable
data class SecretSummary(
    val id: String,
    val label: String,
    val sourceId: String,
    val canReveal: Boolean = false,
    val canManage: Boolean = false,
)

@Serializable
data class SourceView(
    val id: String,
    val spaceId: String,
    val text: String,
    val status: String,
    val createdBy: String,
    val updatedAt: Long,
    val secrets: List<SecretSummary> = emptyList(),
    val error: String? = null,
)

@Serializable data class TaskRequest(val spaceId: String)

@Serializable
data class CompilationTask(
    val id: String,
    val lease: String,
    val source: SourceView,
    val existing: List<MemoryNode>,
    val modelBinding: String?,
    val instructions: String,
)

@Serializable
data class EntryProposal(
    val draft: NodeDraft,
    val existingId: String? = null,
    val baseVersion: Long? = null,
    val secretIds: List<String> = emptyList(),
)

@Serializable
data class RelationProposal(
    val sourceIndex: Int,
    val targetIndex: Int,
    val relation: String,
    val evidence: String = "",
)

@Serializable
data class CompilationResult(
    val entries: List<EntryProposal>,
    val relations: List<RelationProposal> = emptyList(),
    val needsReview: Boolean = false,
)

@Serializable data class TaskSubmission(val lease: String, val result: CompilationResult)

@Serializable data class TaskFailure(val lease: String, val code: String = "model_unavailable")

@Serializable
data class ReviewRequest(val accept: Boolean, val versions: Map<String, Long> = emptyMap())

@Serializable data class TaskList(val sources: List<SourceView>)

@Serializable data class SourceList(val sources: List<SourceView>, val truncated: Boolean)
