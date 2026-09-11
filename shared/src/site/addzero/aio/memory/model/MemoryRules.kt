package site.addzero.aio.memory.model

fun NodeDraft.validated(): NodeDraft {
    fun check(value: Boolean, message: String) { if (!value) throw InputFailure(message) }
    val name = title.trim()
    check(name.isNotEmpty() && name.length <= 160, "标题须为 1 至 160 个字符")
    check(content.length <= 100_000, "正文不能超过 100000 个字符")
    check(url.length <= 2048 && (url.isBlank() || url.startsWith("https://") || url.startsWith("http://")), "来源地址须为 HTTP 或 HTTPS URL")
    check(tags.size <= 12 && tags.all { it.trim().length in 1..32 }, "最多 12 个标签，每个不超过 32 个字符")
    return copy(title = name, url = url.trim(), tags = tags.map(String::trim).distinct())
}

fun EdgeDraft.validated(): EdgeDraft {
    requireId(source)
    requireId(target)
    if (source == target) throw InputFailure("关系的两个节点不能相同")
    if (relation.trim().length !in 1..48) throw InputFailure("关系名称须为 1 至 48 个字符")
    if (evidence.length > 2000) throw InputFailure("关系依据不能超过 2000 个字符")
    return copy(relation = relation.trim())
}

fun requireId(id: String) {
    if (id.length != 32 || id.any { it !in '0'..'9' && it !in 'a'..'f' }) throw InputFailure("节点或关系 ID 无效")
}

/** 双向链接只解析显式的 [[标题]]，不把推测当成模型提取的事实。 */
fun wikiLinks(text: String): List<String> = Regex("\\[\\[([^\\[\\]\\n]{1,160})]]")
    .findAll(text).map { it.groupValues[1].substringBefore('|').trim() }
    .filter(String::isNotEmpty).distinct().take(40).toList()

fun neighborhood(seeds: Set<String>, edges: List<MemoryEdge>, depth: Int, limit: Int = 24): Set<String> {
    val visited = seeds.take(limit).toMutableSet()
    var frontier = visited.toSet()
    repeat(depth.coerceIn(0, 3)) {
        val next = edges.flatMap { edge ->
            when { edge.source in frontier -> listOf(edge.target); edge.target in frontier -> listOf(edge.source); else -> emptyList() }
        }.filter { it !in visited }.distinct().take((limit - visited.size).coerceAtLeast(0)).toSet()
        visited.addAll(next)
        frontier = next
    }
    return visited
}
