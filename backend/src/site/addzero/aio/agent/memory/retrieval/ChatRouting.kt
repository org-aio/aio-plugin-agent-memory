package site.addzero.aio.agent.memory.retrieval

import site.addzero.aio.agent.memory.intake.SourceView
import site.addzero.aio.agent.memory.model.*
import site.addzero.aio.agent.memory.routing.*
import site.addzero.aio.agent.memory.storage.MemoryStore

internal fun MemoryStore.route(source: SourceView): ChatRoute {
    if (source.spaceId != spaceId) throw MissingRecord()
    if (source.status == "quarantined")
        return ChatRoute("quarantined", "已收下，资料已保密暂存。可以在这里补充这段资料的字段用途。")
    val decision = ChatClassifier.classify(source.text)
    if (decision.intent == ChatIntent.SAVE)
        return ChatRoute(
            "save",
            if (access.require(spaceId).modelBinding == null) "已收下，资料已保存。模型配置完成后继续整理。"
            else "已收下，正在后台整理。",
            matchedNodeIds = listOf(source.id),
            activatedNodeIds = listOf(source.id),
        )
    val recalled = recall(RecallRequest(decision.query, limit = 4, excludeIds = listOf(source.id)))
    val matched = recalled.nodes.map { it.id }
    if (decision.intent == ChatIntent.RECALL) {
        val included =
            (matched + links(matched).flatMap { listOf(it.source, it.target) })
                .distinct()
                .filter { it != source.id }
                .take(24)
        val citations = included.map { get(it) }.map { MemoryReference(it.id, it.title) }
        val reply =
            if (matched.isEmpty()) "当前空间没有找到相关资料。"
            else
                "找到 ${matched.size} 条相关资料：\n\n" +
                    recalled.nodes.joinToString("\n\n") { "${it.title}\n${it.content}" }
        return ChatRoute(
            "recall",
            reply,
            citations = citations,
            matchedNodeIds = matched,
            activatedNodeIds = included,
        )
    }
    val context =
        if (matched.isEmpty()) null
        else exportContext(ContextRequest(matched, depth = 1, maxCharacters = 6000))
    val included = context?.nodeIds.orEmpty().filter { it != source.id }
    val citations = included.map { get(it) }.map { MemoryReference(it.id, it.title) }
    return ChatRoute(
        "model",
        context = context?.markdown.orEmpty(),
        citations = citations,
        matchedNodeIds = matched.filter { it in included },
        activatedNodeIds = included,
    )
}

internal fun MemoryStore.activation(request: ActivationRequest): MemoryGraph {
    if (request.nodeIds.size > 24) throw InputFailure("一次最多激活 24 个节点")
    val seeds = visibility(request.nodeIds)
    val near = if (seeds.isEmpty()) emptyList() else links(seeds)
    val focused = (seeds + near.flatMap { listOf(it.source, it.target) }).distinct().take(48)
    val overview = graph(SearchRequest(limit = 120))
    val nodes =
        (focused.map { get(it).copy(content = "") } + overview.nodes.map { it.copy(content = "") })
            .distinctBy { it.id }
            .take(120)
    val edges = links(nodes.map { it.id }, internalOnly = true)
    return MemoryGraph(
        nodes,
        edges.take(800),
        maxOf(overview.total, nodes.size.toLong()),
        overview.truncated || edges.size > 800 || near.size > 800 || focused.size == 48,
    )
}
