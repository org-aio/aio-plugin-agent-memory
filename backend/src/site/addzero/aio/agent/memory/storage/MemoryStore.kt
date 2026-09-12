package site.addzero.aio.agent.memory.storage

import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import site.addzero.aio.agent.memory.access.*
import site.addzero.aio.agent.memory.bindings.Database
import site.addzero.aio.agent.memory.intake.SecretIsolation
import site.addzero.aio.agent.memory.model.*

internal class MemoryStore(val db: DatabaseSession, val access: SpaceAccess, val spaceId: String) {
    private val visible =
        "EXISTS (SELECT 1 FROM plugin_memory_ownership o WHERE o.node_id=n.id AND o.space_id=\$1) AND NOT EXISTS (SELECT 1 FROM plugin_memory_sources s WHERE s.id=n.id AND s.status='deleted') AND NOT EXISTS (SELECT 1 FROM plugin_memory_evidence e JOIN plugin_memory_sources s ON s.id=e.source_id WHERE e.node_id=n.id AND s.status='deleted')"
    private val columns = "n.id,n.title,n.kind,n.content,n.url,n.tags,n.version,n.updated_at"

    fun visibility(ids: List<String>): List<String> {
        access.require(spaceId)
        if (ids.size > 400) throw InputFailure("引用数量超出限制")
        ids.forEach(::requireId)
        if (ids.isEmpty()) return emptyList()
        return ids.chunked(200)
            .flatMap { batch ->
                val slots = batch.indices.joinToString(",") { "\$${it + 2}" }
                db.query(
                        "SELECT n.id FROM plugin_memory_nodes n WHERE $visible AND n.id IN ($slots)",
                        listOf(text(spaceId)) + batch.map(::text),
                    )
                    .map { it.string(0) }
            }
            .distinct()
    }

    fun get(id: String): MemoryNode {
        requireId(id)
        access.require(spaceId)
        return db.query(
                "SELECT $columns FROM plugin_memory_nodes n WHERE $visible AND n.id=\$2",
                listOf(text(spaceId), text(id)),
            )
            .firstOrNull()
            ?.let(::node) ?: throw MissingRecord()
    }

    fun save(
        draft: NodeDraft,
        existing: String? = null,
        author: String = "human",
        sourceId: String? = null,
    ): MemoryNode {
        access.require(spaceId, write = true)
        val value = draft.validated()
        if (
            author != "source" &&
                (value.kind == NodeKind.SOURCE ||
                    existing?.let { get(it).kind == NodeKind.SOURCE } == true)
        )
            throw InputFailure("来源只能通过收件管线新增或修订")
        if (author != "source") {
            val references =
                db.query(
                        "SELECT id FROM plugin_memory_secrets WHERE space_id=\$1",
                        listOf(text(spaceId)),
                    )
                    .map { it.string(0) }
                    .toSet()
            val checked = SecretIsolation.isolate(Json.encodeToString(value), db::id, references)
            if (checked.quarantined || checked.secrets.isNotEmpty())
                throw InputFailure("笔记含疑似秘密，请通过对话或资料收件保存")
        }
        val id = existing ?: db.id()
        val values =
            listOf(
                text(id),
                text(value.title),
                text(value.kind.name),
                text(value.content),
                text(value.url),
                Database.Value.Json(Json.encodeToString(value.tags)),
                number(db.now()),
            )
        if (existing == null) {
            db.execute(
                "INSERT INTO plugin_memory_nodes(id,title,kind,content,url,tags,updated_at) VALUES(\$1,\$2,\$3,\$4,\$5,\$6,\$7)",
                values,
            )
            db.execute(
                "INSERT INTO plugin_memory_ownership(node_id,space_id,created_by,author_type) VALUES(\$1,\$2,\$3,\$4)",
                listOf(text(id), text(spaceId), text(access.user), text(author)),
            )
        } else {
            get(id)
            val version = value.version ?: throw InputFailure("编辑时必须携带当前版本")
            val changed =
                db.execute(
                    "UPDATE plugin_memory_nodes SET title=\$2,kind=\$3,content=\$4,url=\$5,tags=\$6,updated_at=\$7,version=version+1 WHERE id=\$1 AND version=\$8",
                    values + number(version),
                )
            if (changed == 0UL) throw StaleRecord()
            db.execute(
                "UPDATE plugin_memory_ownership SET author_type=\$2 WHERE node_id=\$1",
                listOf(text(id), text(author)),
            )
        }
        db.execute("DELETE FROM plugin_memory_aliases WHERE node_id=\$1", listOf(text(id)))
        value.aliases.forEach { alias ->
            db.execute(
                "INSERT INTO plugin_memory_aliases(node_id,alias) VALUES(\$1,\$2)",
                listOf(text(id), text(alias)),
            )
        }
        val result = get(id)
        val revisionId = db.id()
        db.execute(
            "INSERT INTO plugin_memory_revisions(id,node_id,version,draft,author_id,author_type,source_id,updated_at) VALUES(\$1,\$2,\$3,\$4,\$5,\$6,\$7,\$8)",
            listOf(
                text(revisionId),
                text(id),
                number(result.version),
                Database.Value.Json(Json.encodeToString(value)),
                text(access.user),
                text(author),
                sourceId?.let(::text) ?: Database.Value.Null,
                number(db.now()),
            ),
        )
        sourceId?.let { source ->
            db.execute(
                "INSERT INTO plugin_memory_revision_sources(revision_id,source_id,source_version) VALUES(\$1,\$2,\$3)",
                listOf(text(revisionId), text(source), number(get(source).version)),
            )
        }
        return result
    }

    fun deleteNode(id: String) {
        access.require(spaceId, write = true)
        get(id)
        val source =
            db.query("SELECT id FROM plugin_memory_sources WHERE id=\$1", listOf(text(id)))
                .isNotEmpty()
        if (source) {
            db.execute(
                "UPDATE plugin_memory_sources SET status='deleted',ciphertext=\$2,updated_at=\$3 WHERE id=\$1",
                listOf(text(id), Database.Value.Bytes(emptyList()), number(db.now())),
            )
            db.execute(
                "UPDATE plugin_memory_nodes SET content='',title='已删除来源',version=version+1 WHERE id=\$1",
                listOf(text(id)),
            )
            db.execute("DELETE FROM plugin_memory_secrets WHERE source_id=\$1", listOf(text(id)))
            db.execute(
                "UPDATE plugin_memory_tasks SET state='cancelled',lease=NULL,result=NULL WHERE id=\$1",
                listOf(text(id)),
            )
            db.execute("DELETE FROM plugin_memory_revisions WHERE node_id=\$1", listOf(text(id)))
        } else db.execute("DELETE FROM plugin_memory_nodes WHERE id=\$1", listOf(text(id)))
    }

    fun saveEdge(input: EdgeDraft): MemoryEdge {
        access.require(spaceId, write = true)
        val value = input.validated()
        get(value.source)
        get(value.target)
        val checked = SecretIsolation.isolate(value.relation + "\n" + value.evidence, db::id)
        if (checked.quarantined || checked.secrets.isNotEmpty()) throw InputFailure("关系依据不能包含秘密")
        return db.query(
                "INSERT INTO plugin_memory_edges(id,source,target,relation,evidence) VALUES(\$1,\$2,\$3,\$4,\$5) ON CONFLICT(source,target,relation) DO UPDATE SET evidence=EXCLUDED.evidence RETURNING id,source,target,relation,evidence",
                listOf(
                    text(db.id()),
                    text(value.source),
                    text(value.target),
                    text(value.relation),
                    text(value.evidence),
                ),
            )
            .first()
            .let(::edge)
    }

    fun deleteEdge(id: String) {
        requireId(id)
        access.require(spaceId, write = true)
        val row =
            db.query("SELECT source,target FROM plugin_memory_edges WHERE id=\$1", listOf(text(id)))
                .firstOrNull() ?: throw MissingRecord()
        get(row.string(0))
        get(row.string(1))
        db.execute("DELETE FROM plugin_memory_edges WHERE id=\$1", listOf(text(id)))
    }

    fun graph(search: SearchRequest, includeEdges: Boolean = true): MemoryGraph {
        access.require(spaceId)
        if (search.query.length > 256 || search.limit !in 1..200) throw InputFailure("搜索条件超出限制")
        val pattern =
            "%" + search.query.replace("\\", "\\\\").replace("%", "\\%").replace("_", "\\_") + "%"
        val params = listOf(text(spaceId), text(pattern), text(search.kind?.name ?: ""))
        val where =
            "$visible AND NOT EXISTS (SELECT 1 FROM plugin_memory_sources s WHERE s.id=n.id AND s.status='recorded') AND (n.title ILIKE \$2 OR n.content ILIKE \$2 OR n.tags::text ILIKE \$2 OR EXISTS (SELECT 1 FROM plugin_memory_aliases a WHERE a.node_id=n.id AND a.alias ILIKE \$2)) AND (\$3='' OR n.kind=\$3)"
        val count =
            db.query("SELECT count(*) FROM plugin_memory_nodes n WHERE $where", params)
                .first()
                .long(0)
        val nodes =
            db.query(
                    "SELECT n.id,n.title,n.kind,substring(n.content,1,180),n.url,n.tags,n.version,n.updated_at FROM plugin_memory_nodes n WHERE $where ORDER BY n.updated_at DESC,n.id LIMIT \$4",
                    params + number(search.limit.toLong()),
                )
                .map(::node)
        val edges = if (includeEdges) links(nodes.map { it.id }, internalOnly = true) else emptyList()
        return MemoryGraph(nodes, edges.take(800), count, count > nodes.size || edges.size > 800)
    }

    fun links(ids: List<String>, internalOnly: Boolean = false): List<MemoryEdge> {
        if (ids.isEmpty()) return emptyList()
        if (ids.size > 200) throw InputFailure("一次最多查询 200 个节点")
        if (!visibility(ids).toSet().containsAll(ids)) throw MissingRecord()
        val slots = ids.indices.joinToString(",") { "\$${it + 2}" }
        val join = if (internalOnly) "AND" else "OR"
        return db.query(
                "SELECT e.id,e.source,e.target,e.relation,e.evidence FROM plugin_memory_edges e WHERE (e.source IN ($slots) $join e.target IN ($slots)) AND e.source IN (SELECT n.id FROM plugin_memory_nodes n WHERE $visible) AND e.target IN (SELECT n.id FROM plugin_memory_nodes n WHERE $visible) ORDER BY e.id LIMIT 801",
                listOf(text(spaceId)) + ids.map(::text),
            )
            .map(::edge)
    }

    fun author(id: String): String {
        get(id)
        return db.query(
                "SELECT author_type FROM plugin_memory_ownership WHERE node_id=\$1",
                listOf(text(id)),
            )
            .first()
            .string(0)
    }

    fun revisions(id: String): List<WikiRevision> {
        get(id)
        return db.query(
                "SELECT version,draft,author_id,author_type,source_id,updated_at FROM plugin_memory_revisions WHERE node_id=\$1 ORDER BY version DESC LIMIT 100",
                listOf(text(id)),
            )
            .map {
                WikiRevision(
                    it.long(0),
                    Json.decodeFromString(it.string(1)),
                    it.string(2),
                    it.string(3),
                    it.nullableString(4),
                    it.long(5),
                )
            }
    }

    fun rollback(id: String, request: RollbackRequest): MemoryNode {
        val current = get(id)
        if (current.version != request.currentVersion) throw StaleRecord()
        val revision =
            revisions(id).firstOrNull { it.version == request.version } ?: throw MissingRecord()
        return save(revision.draft.copy(version = current.version), id)
    }

    fun evidence(id: String): List<String> {
        get(id)
        return db.query(
                "SELECT source_id FROM plugin_memory_evidence WHERE node_id=\$1",
                listOf(text(id)),
            )
            .map { it.string(0) }
    }

    private fun node(row: List<Database.Value>) =
        MemoryNode(
            row.string(0),
            row.string(1),
            NodeKind.valueOf(row.string(2)),
            row.string(3),
            row.string(4),
            Json.decodeFromString(row.string(5)),
            row.long(6),
            row.long(7),
            db.query(
                    "SELECT alias FROM plugin_memory_aliases WHERE node_id=\$1 ORDER BY alias",
                    listOf(text(row.string(0))),
                )
                .map { it.string(0) },
        )

    private fun edge(row: List<Database.Value>) =
        MemoryEdge(row.string(0), row.string(1), row.string(2), row.string(3), row.string(4))
}
