package site.addzero.aio.agent.memory.routing

import kotlinx.serialization.Serializable

enum class ChatIntent {
    SAVE,
    RECALL,
    MODEL,
}

data class RoutingDecision(val intent: ChatIntent, val query: String)

@Serializable data class RouteRequest(val sourceId: String)

@Serializable data class MemoryReference(val id: String, val title: String)

@Serializable
data class ChatRoute(
    val route: String,
    val reply: String? = null,
    val context: String = "",
    val citations: List<MemoryReference> = emptyList(),
    val matchedNodeIds: List<String> = emptyList(),
    val activatedNodeIds: List<String> = emptyList(),
)

@Serializable data class ActivationRequest(val nodeIds: List<String> = emptyList())
