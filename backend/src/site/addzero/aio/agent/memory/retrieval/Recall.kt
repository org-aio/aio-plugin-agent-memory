package site.addzero.aio.agent.memory.retrieval

import site.addzero.aio.agent.memory.model.*
import site.addzero.aio.agent.memory.storage.MemoryStore

internal fun MemoryStore.recall(request: RecallRequest): MemoryGraph {
    if (request.query.length > 100_000 || request.limit !in 1..24) throw InputFailure("检索条件无效")
    val question = request.query.replace(Regex("\\[\\[secret:[a-f0-9]{32}]]"), "")
    val words = question.split(Regex("[^A-Za-z0-9_\\u4e00-\\u9fff]+")).filter { it.length in 2..80 }
    val stop =
        setOf(
            "帮我",
            "一下",
            "找下",
            "记下",
            "密码",
            "什么",
            "我的",
            "怎么",
            "这个",
            "是啥",
            "secret",
            "password",
            "username",
            "token",
        )
    val terms =
        (words +
                words.flatMap { word ->
                    if (word.any { it in '\u4e00'..'\u9fff' }) word.windowed(2) else emptyList()
                })
            .filter { it !in stop }
            .distinct()
            .take(12)
    val scores = mutableMapOf<String, Int>()
    val matches = linkedMapOf<String, MemoryNode>()
    for (term in terms) {
        graph(SearchRequest(term, limit = 24)).nodes.forEach { node ->
            matches[node.id] = node
            scores[node.id] = (scores[node.id] ?: 0) + if (node.title.contains(term)) 3 else 1
        }
    }
    val nodes = matches.values.sortedByDescending { scores[it.id] }.take(request.limit)
    return MemoryGraph(
        nodes,
        links(nodes.map { it.id }, internalOnly = true),
        matches.size.toLong(),
        matches.size > nodes.size,
    )
}
