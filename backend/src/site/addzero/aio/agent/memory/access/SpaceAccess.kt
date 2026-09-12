package site.addzero.aio.agent.memory.access

import site.addzero.aio.agent.memory.bindings.Host
import site.addzero.aio.agent.memory.model.*
import site.addzero.aio.agent.memory.storage.*

internal class SpaceAccess(private val db: DatabaseSession) {
    val user = Host.context().userId?.takeIf { it.isNotBlank() } ?: throw AccessDenied()

    fun list(): List<MemorySpace> {
        personal()
        return db.query(
                "SELECT s.id,s.title,s.personal_owner IS NOT NULL,m.role,s.model_binding FROM plugin_memory_spaces s JOIN plugin_memory_members m ON m.space_id=s.id WHERE m.user_id=\$1 ORDER BY s.personal_owner IS NULL,s.title",
                listOf(text(user)),
            )
            .map(::space)
    }

    fun personal(): MemorySpace {
        val existing =
            db.query(
                    "SELECT id FROM plugin_memory_spaces WHERE personal_owner=\$1",
                    listOf(text(user)),
                )
                .firstOrNull()
        if (existing != null) return require(existing.string(0))
        val rows =
            db.query(
                "INSERT INTO plugin_memory_spaces(id,title,personal_owner,created_by) VALUES(\$1,'个人记忆',\$2,\$2) ON CONFLICT(personal_owner) DO UPDATE SET personal_owner=EXCLUDED.personal_owner RETURNING id",
                listOf(text(db.id()), text(user)),
            )
        val id = rows.first().string(0)
        db.execute(
            "INSERT INTO plugin_memory_members(space_id,user_id,role) VALUES(\$1,\$2,'OWNER') ON CONFLICT DO NOTHING",
            listOf(text(id), text(user)),
        )
        return require(id)
    }

    fun require(id: String?, write: Boolean = false, owner: Boolean = false): MemorySpace {
        if (id == null) return require(personal().id, write, owner)
        requireId(id)
        val row =
            db.query(
                    "SELECT s.id,s.title,s.personal_owner IS NOT NULL,m.role,s.model_binding FROM plugin_memory_spaces s JOIN plugin_memory_members m ON m.space_id=s.id WHERE s.id=\$1 AND m.user_id=\$2 FOR SHARE OF m",
                    listOf(text(id), text(user)),
                )
                .firstOrNull() ?: throw AccessDenied()
        val space = space(row)
        if ((write && space.role == SpaceRole.READER) || (owner && space.role != SpaceRole.OWNER))
            throw AccessDenied()
        return space
    }

    fun node(id: String, write: Boolean = false): MemorySpace {
        requireId(id)
        val row =
            db.query(
                    "SELECT space_id FROM plugin_memory_ownership WHERE node_id=\$1",
                    listOf(text(id)),
                )
                .firstOrNull() ?: throw MissingRecord()
        return require(row.string(0), write)
    }

    fun save(draft: SpaceDraft, id: String? = null): MemorySpace {
        if (draft.title.trim().length !in 1..80 || (draft.modelBinding?.length ?: 0) > 160)
            throw InputFailure("空间配置无效")
        val binding =
            draft.modelBinding?.let(::text)
                ?: site.addzero.aio.agent.memory.bindings.Database.Value.Null
        if (id != null) {
            require(id, owner = true)
            db.execute(
                "UPDATE plugin_memory_spaces SET title=\$2,model_binding=\$3 WHERE id=\$1",
                listOf(text(id), text(draft.title.trim()), binding),
            )
        } else {
            val created = db.id()
            db.execute(
                "INSERT INTO plugin_memory_spaces(id,title,created_by,model_binding) VALUES(\$1,\$2,\$3,\$4)",
                listOf(text(created), text(draft.title.trim()), text(user), binding),
            )
            db.execute(
                "INSERT INTO plugin_memory_members(space_id,user_id,role) VALUES(\$1,\$2,'OWNER')",
                listOf(text(created), text(user)),
            )
            return require(created)
        }
        return require(id)
    }

    fun members(id: String): List<SpaceMember> {
        require(id, owner = true)
        return db.query(
                "SELECT user_id,role FROM plugin_memory_members WHERE space_id=\$1 ORDER BY user_id",
                listOf(text(id)),
            )
            .map { SpaceMember(it.string(0), SpaceRole.valueOf(it.string(1))) }
    }

    fun member(id: String, draft: MemberDraft?, target: String) {
        val space = require(id, owner = true)
        if (space.personal || target.isBlank() || target.length > 128 || target == user)
            throw InputFailure("不能更改个人空间或自己的成员权限")
        if (draft == null) {
            db.execute(
                "DELETE FROM plugin_memory_members WHERE space_id=\$1 AND user_id=\$2",
                listOf(text(id), text(target)),
            )
            db.execute(
                "DELETE FROM plugin_memory_secret_grants WHERE user_id=\$2 AND secret_id IN (SELECT id FROM plugin_memory_secrets WHERE space_id=\$1)",
                listOf(text(id), text(target)),
            )
        } else
            db.execute(
                "INSERT INTO plugin_memory_members(space_id,user_id,role) VALUES(\$1,\$2,\$3) ON CONFLICT(space_id,user_id) DO UPDATE SET role=EXCLUDED.role",
                listOf(text(id), text(target), text(draft.role.name)),
            )
    }

    fun canWriteAs(space: String, actor: String): Boolean =
        db.query(
                "SELECT role FROM plugin_memory_members WHERE space_id=\$1 AND user_id=\$2 AND role IN ('OWNER','EDITOR') FOR SHARE",
                listOf(text(space), text(actor)),
            )
            .isNotEmpty()

    private fun space(row: List<site.addzero.aio.agent.memory.bindings.Database.Value>) =
        MemorySpace(
            row.string(0),
            row.string(1),
            row.boolean(2),
            SpaceRole.valueOf(row.string(3)),
            row.nullableString(4),
        )
}
