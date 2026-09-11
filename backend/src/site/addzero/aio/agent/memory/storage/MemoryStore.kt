package site.addzero.aio.agent.memory.storage

import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import site.addzero.aio.agent.memory.bindings.Database
import site.addzero.aio.agent.memory.bindings.Host
import site.addzero.aio.agent.memory.model.*

internal class MemoryStore(private val tx: UInt) {
    companion object {
        fun <T> transaction(block: (MemoryStore) -> T): T {
            val tx = Database.begin().getOrThrow()
            return try {
                val value = block(MemoryStore(tx))
                Database.finish(tx, true).getOrThrow()
                value
            } catch (failure: Throwable) {
                Database.finish(tx, false)
                throw failure
            }
        }
    }

    private fun text(value: String) = Database.Value.Text(value)
    private fun number(value: Long) = Database.Value.Integer(value)
    private fun query(sql: String, values: List<Database.Value> = emptyList()) = Database.query(tx, sql, values).getOrThrow().values
    private fun execute(sql: String, values: List<Database.Value>) = Database.execute(tx, sql, values).getOrThrow()
    private fun id() = Host.random(16U).getOrThrow().joinToString("") { it.toString(16).padStart(2, '0') }
    private fun List<Database.Value>.string(index: Int) = when (val value = this[index]) {
        is Database.Value.Text -> value.value
        is Database.Value.Json -> value.value
        else -> error("数据库字段类型不匹配")
    }
    private fun List<Database.Value>.long(index: Int) = (this[index] as Database.Value.Integer).value
    private fun node(row: List<Database.Value>) = MemoryNode(
        row.string(0), row.string(1), NodeKind.valueOf(row.string(2)), row.string(3), row.string(4),
        Json.decodeFromString(row.string(5)), row.long(6), row.long(7),
    )
    private fun edge(row: List<Database.Value>) = MemoryEdge(row.string(0), row.string(1), row.string(2), row.string(3), row.string(4))

    fun total(): Long = query("SELECT count(*) FROM plugin_memory_nodes").first().long(0)

    fun get(id: String): MemoryNode {
        requireId(id)
        return query("SELECT id,title,kind,content,url,tags,version,updated_at FROM plugin_memory_nodes WHERE id=\$1", listOf(text(id)))
            .firstOrNull()?.let(::node) ?: throw MissingRecord()
    }

    fun save(draft: NodeDraft, existing: String? = null): MemoryNode {
        val value = draft.validated()
        val id = existing ?: id()
        val values = listOf(text(id), text(value.title), text(value.kind.name), text(value.content), text(value.url),
            Database.Value.Json(Json.encodeToString(value.tags)), number(Host.now().toLong()))
        if (existing == null) {
            execute("INSERT INTO plugin_memory_nodes(id,title,kind,content,url,tags,updated_at) VALUES (\$1,\$2,\$3,\$4,\$5,\$6,\$7)", values)
        } else {
            requireId(id)
            val version = value.version ?: throw InputFailure("编辑时必须携带当前版本")
            val changed = execute("UPDATE plugin_memory_nodes SET title=\$2,kind=\$3,content=\$4,url=\$5,tags=\$6,updated_at=\$7,version=version+1 WHERE id=\$1 AND version=\$8", values + number(version))
            if (changed == 0UL) { get(id); throw StaleRecord() }
        }
        return get(id)
    }

    fun deleteNode(id: String) {
        requireId(id)
        if (execute("DELETE FROM plugin_memory_nodes WHERE id=\$1", listOf(text(id))) == 0UL) throw MissingRecord()
    }

    fun saveEdge(input: EdgeDraft): MemoryEdge {
        val value = input.validated()
        get(value.source)
        get(value.target)
        val result = query("INSERT INTO plugin_memory_edges(id,source,target,relation,evidence) VALUES (\$1,\$2,\$3,\$4,\$5) ON CONFLICT (source,target,relation) DO UPDATE SET evidence=EXCLUDED.evidence RETURNING id,source,target,relation,evidence",
            listOf(text(id()), text(value.source), text(value.target), text(value.relation), text(value.evidence)))
        return edge(result.first())
    }

    fun deleteEdge(id: String) {
        requireId(id)
        if (execute("DELETE FROM plugin_memory_edges WHERE id=\$1", listOf(text(id))) == 0UL) throw MissingRecord()
    }

    fun graph(search: SearchRequest): MemoryGraph {
        if (search.query.length > 256 || search.limit !in 1..200) throw InputFailure("搜索条件超出限制")
        val pattern = "%" + search.query.replace("\\", "\\\\").replace("%", "\\%").replace("_", "\\_") + "%"
        val params = listOf(text(pattern), text(search.kind?.name ?: ""))
        val where = "(title ILIKE \$1 OR content ILIKE \$1 OR tags::text ILIKE \$1) AND (\$2='' OR kind=\$2)"
        val count = query("SELECT count(*) FROM plugin_memory_nodes WHERE $where", params).first().long(0)
        val nodes = query("SELECT id,title,kind,substring(content,1,180),url,tags,version,updated_at FROM plugin_memory_nodes WHERE $where ORDER BY updated_at DESC,id LIMIT \$3", params + number(search.limit.toLong())).map(::node)
        val edges = links(nodes.map { it.id }, internalOnly = true)
        return MemoryGraph(nodes, edges.take(800), count, count > nodes.size || edges.size > 800)
    }

    fun links(ids: List<String>, internalOnly: Boolean = false): List<MemoryEdge> {
        if (ids.isEmpty()) return emptyList()
        if (ids.size > 200) throw InputFailure("一次最多查询 200 个节点")
        ids.forEach(::requireId)
        val slots = ids.indices.joinToString(",") { "\$${it + 1}" }
        val join = if (internalOnly) "AND" else "OR"
        return query("SELECT id,source,target,relation,evidence FROM plugin_memory_edges WHERE source IN ($slots) $join target IN ($slots) ORDER BY id LIMIT 801", ids.map(::text)).map(::edge)
    }

    fun importSource(input: ImportRequest): ImportResult {
        val source = save(NodeDraft(input.title, NodeKind.SOURCE, input.text, input.url))
        val links = wikiLinks(input.text).filter { !it.equals(source.title, true) }
        links.forEach { title ->
            val existing = query("SELECT id FROM plugin_memory_nodes WHERE lower(title)=lower(\$1) AND kind='CONCEPT' ORDER BY id LIMIT 1", listOf(text(title))).firstOrNull()?.string(0)
            val concept = existing ?: save(NodeDraft(title, NodeKind.CONCEPT)).id
            saveEdge(EdgeDraft(source.id, concept, "提及", "[[${title}]]"))
        }
        return ImportResult(source, links.size)
    }
}
