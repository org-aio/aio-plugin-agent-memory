package site.addzero.aio.agent.memory.access

import kotlinx.serialization.Serializable

@Serializable
enum class SpaceRole {
    OWNER,
    EDITOR,
    READER,
}

@Serializable
data class MemorySpace(
    val id: String,
    val title: String,
    val personal: Boolean,
    val role: SpaceRole,
    val modelBinding: String? = null,
)

@Serializable data class SpaceDraft(val title: String, val modelBinding: String? = null)

@Serializable data class MemberDraft(val userId: String, val role: SpaceRole)

@Serializable data class SpaceMember(val userId: String, val role: SpaceRole)

@Serializable
data class SecretGrant(val userId: String, val reveal: Boolean = true, val manage: Boolean = false)

@Serializable data class RevealedSecret(val value: String)

class AccessDenied : IllegalStateException("没有访问权限")
