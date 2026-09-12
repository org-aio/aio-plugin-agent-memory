package site.addzero.aio.agent.memory.intake

import site.addzero.aio.agent.memory.access.*
import site.addzero.aio.agent.memory.bindings.Cryptography
import site.addzero.aio.agent.memory.bindings.Database
import site.addzero.aio.agent.memory.bindings.Host
import site.addzero.aio.agent.memory.model.*
import site.addzero.aio.agent.memory.storage.*

internal class IntakeStore(private val db: DatabaseSession, private val access: SpaceAccess) {
    fun capture(request: CaptureRequest): SourceView {
        if (
            request.text.isBlank() ||
                request.text.encodeToByteArray().size > 100_000 ||
                request.requestId.length !in 1..160 ||
                request.origin !in setOf("chat", "note", "import") ||
                request.reference.length > 256
        )
            throw InputFailure("收件内容或来源无效")
        val space = access.require(request.spaceId, write = true)
        val existing =
            db.query(
                    "SELECT id,ciphertext,status FROM plugin_memory_sources WHERE space_id=\$1 AND created_by=\$2 AND request_id=\$3 FOR UPDATE",
                    listOf(text(space.id), text(access.user), text(request.requestId)),
                )
                .firstOrNull()
        if (existing != null) {
            if (existing.string(2) == "deleted") throw StaleRecord()
            val original = open("${space.id}/source/${existing.string(0)}", existing.bytes(1))
            if (original != request.text) throw InputFailure("同一请求 ID 不能提交不同内容")
            return source(existing.string(0))
        }
        val isolated = SecretIsolation.isolate(request.text, db::id)
        val store = MemoryStore(db, access, space.id)
        val title = if (isolated.quarantined) "待整理的保密资料" else sourceTitle(isolated.text)
        val node = store.save(NodeDraft(title, NodeKind.SOURCE, isolated.text), author = "source")
        val status = if (isolated.quarantined) "quarantined" else "pending"
        db.execute(
            "INSERT INTO plugin_memory_sources(id,space_id,created_by,request_id,ciphertext,status,origin,reference,updated_at) VALUES(\$1,\$2,\$3,\$4,\$5,\$6,\$7,\$8,\$9)",
            listOf(
                text(node.id),
                text(space.id),
                text(access.user),
                text(request.requestId),
                seal("${space.id}/source/${node.id}", request.text),
                text(status),
                text(request.origin),
                text(request.reference),
                number(db.now()),
            ),
        )
        isolated.secrets.forEach { secret ->
            db.execute(
                "INSERT INTO plugin_memory_secrets(id,source_id,space_id,label,ciphertext,owner_id) VALUES(\$1,\$2,\$3,\$4,\$5,\$6)",
                listOf(
                    text(secret.id),
                    text(node.id),
                    text(space.id),
                    text(secret.label),
                    seal("${space.id}/secret/${secret.id}", secret.value),
                    text(access.user),
                ),
            )
        }
        if (!isolated.quarantined) {
            db.execute(
                "INSERT INTO plugin_memory_tasks(id,space_id,actor_id,state,available_at) VALUES(\$1,\$2,\$3,'pending',\$4)",
                listOf(text(node.id), text(space.id), text(access.user), number(db.now())),
            )
            wikiLinks(isolated.text)
                .filter { it != title && !it.startsWith("secret:") }
                .forEach { name ->
                    val concept =
                        store.graph(SearchRequest(query = name)).nodes.firstOrNull {
                            it.title == name && it.kind != NodeKind.SOURCE
                        } ?: store.save(NodeDraft(name, NodeKind.CONCEPT), author = "source")
                    store.saveEdge(EdgeDraft(node.id, concept.id, "提及", "显式双向链接"))
                    db.execute(
                        "INSERT INTO plugin_memory_evidence(node_id,source_id) VALUES(\$1,\$2) ON CONFLICT DO NOTHING",
                        listOf(text(concept.id), text(node.id)),
                    )
                }
        }
        request.clarifies?.let { previous ->
            Clarification.secretLabel(request.text)?.let { label ->
                clarify(previous, node.id, request, label, space.id)
            }
        }
        return source(node.id)
    }

    private fun clarify(
        id: String,
        explanation: String,
        request: CaptureRequest,
        label: String,
        spaceId: String,
    ) {
        requireId(id)
        val row =
            db.query(
                    "SELECT ciphertext FROM plugin_memory_sources WHERE id=\$1 AND space_id=\$2 AND created_by=\$3 AND reference=\$4 AND origin='chat' AND status='quarantined' FOR UPDATE",
                    listOf(text(id), text(spaceId), text(access.user), text(request.reference)),
                )
                .firstOrNull() ?: return
        if (request.origin != "chat" || request.reference.isBlank()) return
        val secret = db.id()
        val original = open("$spaceId/source/$id", row.bytes(0))
        db.execute("DELETE FROM plugin_memory_secrets WHERE source_id=\$1", listOf(text(id)))
        db.execute(
            "INSERT INTO plugin_memory_secrets(id,source_id,space_id,label,ciphertext,owner_id) VALUES(\$1,\$2,\$3,\$4,\$5,\$6)",
            listOf(
                text(secret),
                text(id),
                text(spaceId),
                text(label),
                seal("$spaceId/secret/$secret", original),
                text(access.user),
            ),
        )
        val store = MemoryStore(db, access, spaceId)
        val current = store.get(id)
        val revised =
            store.save(
                NodeDraft(
                    "保密资料",
                    NodeKind.SOURCE,
                    "$label: [[secret:$secret]]",
                    version = current.version,
                ),
                id,
                "source",
            )
        db.execute(
            "INSERT INTO plugin_memory_clarifications(source_id,explanation_id,source_version) VALUES(\$1,\$2,\$3)",
            listOf(text(id), text(explanation), number(revised.version)),
        )
        db.execute(
            "UPDATE plugin_memory_sources SET status='pending',error=NULL,updated_at=\$2 WHERE id=\$1",
            listOf(text(id), number(db.now())),
        )
        db.execute(
            "INSERT INTO plugin_memory_tasks(id,space_id,actor_id,state,available_at) VALUES(\$1,\$2,\$3,'pending',\$4)",
            listOf(text(id), text(spaceId), text(access.user), number(db.now())),
        )
    }

    fun source(id: String): SourceView {
        val space = access.node(id)
        val row =
            db.query(
                    "SELECT s.id,s.space_id,n.content,s.status,s.created_by,s.updated_at,s.error FROM plugin_memory_sources s JOIN plugin_memory_nodes n ON n.id=s.id WHERE s.id=\$1 AND s.status<>'deleted'",
                    listOf(text(id)),
                )
                .firstOrNull() ?: throw MissingRecord()
        return SourceView(
            row.string(0),
            row.string(1),
            row.string(2),
            row.string(3),
            row.string(4),
            row.long(5),
            secrets(space.id, id),
            row.nullableString(6),
        )
    }

    fun list(spaceId: String?): SourceList {
        val space = access.require(spaceId)
        val ids =
            db.query(
                "SELECT id FROM plugin_memory_sources WHERE space_id=\$1 AND status<>'deleted' ORDER BY updated_at DESC,id LIMIT 201",
                listOf(text(space.id)),
            )
        return SourceList(ids.take(200).map { source(it.string(0)) }, ids.size > 200)
    }

    fun original(id: String): RevealedSecret {
        val source = source(id)
        if (source.createdBy != access.user || workerCall()) throw AccessDenied()
        val row =
            db.query("SELECT ciphertext FROM plugin_memory_sources WHERE id=\$1", listOf(text(id)))
                .first()
        return RevealedSecret(open("${source.spaceId}/source/$id", row.bytes(0)))
    }

    fun secrets(spaceId: String, sourceId: String? = null): List<SecretSummary> {
        access.require(spaceId)
        return db.query(
                "SELECT s.id,s.label,s.source_id,(s.owner_id=\$2 OR coalesce(g.can_reveal,false)),(s.owner_id=\$2 OR coalesce(g.can_manage,false)) FROM plugin_memory_secrets s LEFT JOIN plugin_memory_secret_grants g ON g.secret_id=s.id AND g.user_id=\$2 WHERE s.space_id=\$1 AND (\$3='' OR s.source_id=\$3) ORDER BY s.id LIMIT 200",
                listOf(text(spaceId), text(access.user), text(sourceId ?: "")),
            )
            .map {
                SecretSummary(
                    it.string(0),
                    it.string(1),
                    it.string(2),
                    !workerCall() && it.boolean(3),
                    !workerCall() && it.boolean(4),
                )
            }
    }

    fun reveal(id: String): RevealedSecret {
        val record = secret(id)
        if (workerCall() || !record.boolean(4)) throw AccessDenied()
        return RevealedSecret(open("${record.string(0)}/secret/$id", record.bytes(1)))
    }

    fun grant(id: String, grant: SecretGrant) {
        val record = secret(id)
        if (workerCall() || !record.boolean(5)) throw AccessDenied()
        if (grant.userId.isBlank() || grant.userId.length > 128) throw InputFailure("成员无效")
        if (
            db.query(
                    "SELECT user_id FROM plugin_memory_members WHERE space_id=\$1 AND user_id=\$2 FOR SHARE",
                    listOf(text(record.string(0)), text(grant.userId)),
                )
                .isEmpty()
        )
            throw InputFailure("目标用户不是当前空间成员")
        db.execute(
            "INSERT INTO plugin_memory_secret_grants(secret_id,user_id,can_reveal,can_manage) VALUES(\$1,\$2,\$3,\$4) ON CONFLICT(secret_id,user_id) DO UPDATE SET can_reveal=EXCLUDED.can_reveal,can_manage=EXCLUDED.can_manage",
            listOf(text(id), text(grant.userId), bool(grant.reveal), bool(grant.manage)),
        )
    }

    fun retry(id: String): SourceView {
        val source = source(id)
        access.require(source.spaceId, write = true)
        if (source.status == "quarantined") throw InputFailure("请在对话中补充字段名称后重新提交资料")
        if (source.status !in setOf("failed", "conflict", "pending")) throw InputFailure("当前状态不能重试")
        db.execute(
            "UPDATE plugin_memory_tasks SET state='pending',attempts=0,available_at=\$2,lease=NULL,result=NULL,error=NULL WHERE id=\$1",
            listOf(text(id), number(db.now())),
        )
        db.execute(
            "UPDATE plugin_memory_sources SET status='pending',error=NULL WHERE id=\$1",
            listOf(text(id)),
        )
        return source(id)
    }

    private fun secret(id: String): List<Database.Value> {
        requireId(id)
        val row =
            db.query(
                    "SELECT s.space_id,s.ciphertext,s.owner_id,s.source_id,(s.owner_id=\$2 OR coalesce(g.can_reveal,false)),(s.owner_id=\$2 OR coalesce(g.can_manage,false)) FROM plugin_memory_secrets s LEFT JOIN plugin_memory_secret_grants g ON g.secret_id=s.id AND g.user_id=\$2 WHERE s.id=\$1",
                    listOf(text(id), text(access.user)),
                )
                .firstOrNull() ?: throw MissingRecord()
        access.require(row.string(0))
        return row
    }

    private fun seal(purpose: String, value: String) =
        Database.Value.Bytes(
            Cryptography.seal(purpose, value.encodeToByteArray().map { it.toUByte() }).getOrThrow()
        )

    private fun open(purpose: String, value: List<UByte>) =
        Cryptography.open(purpose, value)
            .getOrThrow()
            .map { it.toByte() }
            .toByteArray()
            .decodeToString()

    private fun workerCall() = Host.context().sessionId?.startsWith("service:") == true
}
