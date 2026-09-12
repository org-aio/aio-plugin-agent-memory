package site.addzero.aio.agent.memory.transport

import kotlinx.serialization.SerializationException
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import site.addzero.aio.agent.memory.access.*
import site.addzero.aio.agent.memory.bindings.*
import site.addzero.aio.agent.memory.bindings.PluginRootFunctions
import site.addzero.aio.agent.memory.intake.*
import site.addzero.aio.agent.memory.model.*
import site.addzero.aio.agent.memory.retrieval.exportContext
import site.addzero.aio.agent.memory.retrieval.recall
import site.addzero.aio.agent.memory.storage.DatabaseSession
import site.addzero.aio.agent.memory.storage.MemoryStore

internal object PluginRootFunctionsExportsImpl : PluginRootFunctions.Exports {
    private val wireJson = Json { encodeDefaults = true }

    override fun describe() =
        Metadata.Description(
            "Agent Memory",
            listOf(
                Metadata.PageDefinition(
                    "agent-memory",
                    "记忆图谱",
                    "index.html",
                    Metadata.Scene("community", "社区插件"),
                    listOf("Agent"),
                    null,
                    Metadata.Surface.WORKSPACE,
                )
            ),
        )

    // 健康检查阶段不开放宿主能力；连接和迁移由宿主验证，业务访问在请求事务中执行。
    override fun health(): Result<Unit> = Result.success(Unit)

    override fun lifecycle(phase: Transport.Phase): Result<Unit> = Result.success(Unit)

    override fun handle(request: Transport.Request): Transport.Response {
        val context = Host.context()
        if (context.tenantId.isNullOrBlank() || context.userId.isNullOrBlank())
            return respond(401, Failure("请先登录"))
        if (request.body.size > 512_000) return respond(413, Failure("请求正文过大"))
        return try {
            DatabaseSession.transaction { db ->
                val path = request.path.split('/').filter(String::isNotEmpty)
                val body = request.body.map { it.toByte() }.toByteArray().decodeToString()
                val access = SpaceAccess(db)
                val intake = IntakeStore(db, access)
                val queue = CompilationQueue(db, access, intake)
                val requestedSpace =
                    request.query
                        ?.split('&')
                        ?.firstOrNull { it.startsWith("spaceId=") }
                        ?.substringAfter('=')
                val space =
                    if (path.size >= 2 && path[0] == "nodes") access.node(path[1])
                    else access.require(requestedSpace)
                val store = MemoryStore(db, access, space.id)
                when {
                    request.path == "/spaces" && request.method == "GET" ->
                        respond(200, access.list())
                    request.path == "/spaces" && request.method == "POST" ->
                        respond(201, access.save(Json.decodeFromString<SpaceDraft>(body)))
                    path.size == 2 && path[0] == "spaces" && request.method == "PUT" ->
                        respond(200, access.save(Json.decodeFromString<SpaceDraft>(body), path[1]))
                    path.size == 3 &&
                        path[0] == "spaces" &&
                        path[2] == "members" &&
                        request.method == "GET" -> respond(200, access.members(path[1]))
                    path.size == 3 &&
                        path[0] == "spaces" &&
                        path[2] == "members" &&
                        request.method == "POST" -> {
                        val draft = Json.decodeFromString<MemberDraft>(body)
                        access.member(path[1], draft, draft.userId)
                        empty()
                    }
                    path.size == 4 &&
                        path[0] == "spaces" &&
                        path[2] == "members" &&
                        request.method == "DELETE" -> {
                        access.member(path[1], null, path[3])
                        empty()
                    }
                    request.path == "/capture" && request.method == "POST" ->
                        respond(202, intake.capture(Json.decodeFromString<CaptureRequest>(body)))
                    request.path == "/sources" && request.method == "GET" ->
                        respond(200, intake.list(space.id))
                    path.size == 2 && path[0] == "sources" && request.method == "GET" ->
                        respond(200, intake.source(path[1]))
                    path.size == 3 &&
                        path[0] == "sources" &&
                        path[2] == "original" &&
                        request.method == "POST" -> respond(200, intake.original(path[1]))
                    path.size == 3 &&
                        path[0] == "sources" &&
                        path[2] == "retry" &&
                        request.method == "POST" -> respond(200, intake.retry(path[1]))
                    request.path == "/secrets" && request.method == "GET" ->
                        respond(200, intake.secrets(space.id))
                    path.size == 3 &&
                        path[0] == "secrets" &&
                        path[2] == "reveal" &&
                        request.method == "POST" -> respond(200, intake.reveal(path[1]))
                    path.size == 3 &&
                        path[0] == "secrets" &&
                        path[2] == "grants" &&
                        request.method == "PUT" -> {
                        intake.grant(path[1], Json.decodeFromString<SecretGrant>(body))
                        empty()
                    }
                    request.path == "/tasks/claim" && request.method == "POST" ->
                        respond(200, queue.claim(Json.decodeFromString<TaskRequest>(body).spaceId))
                    path.size == 3 &&
                        path[0] == "tasks" &&
                        path[2] == "submit" &&
                        request.method == "POST" ->
                        respond(
                            200,
                            queue.submit(path[1], Json.decodeFromString<TaskSubmission>(body)),
                        )
                    path.size == 3 &&
                        path[0] == "tasks" &&
                        path[2] == "fail" &&
                        request.method == "POST" ->
                        respond(200, queue.fail(path[1], Json.decodeFromString<TaskFailure>(body)))
                    path.size == 3 &&
                        path[0] == "tasks" &&
                        path[2] == "proposal" &&
                        request.method == "GET" -> respond(200, queue.proposal(path[1]))
                    path.size == 3 &&
                        path[0] == "sources" &&
                        path[2] == "proposal" &&
                        request.method == "GET" -> respond(200, queue.proposal(path[1]))
                    path.size == 3 &&
                        path[0] == "sources" &&
                        path[2] == "resolve" &&
                        request.method == "POST" ->
                        respond(
                            200,
                            queue.resolve(path[1], Json.decodeFromString<ReviewRequest>(body)),
                        )
                    request.path == "/graph" && request.method == "GET" ->
                        respond(200, store.graph(SearchRequest()))
                    request.path == "/search" && request.method == "POST" ->
                        respond(200, store.graph(Json.decodeFromString<SearchRequest>(body)))
                    request.path == "/recall" && request.method == "POST" ->
                        respond(200, store.recall(Json.decodeFromString<RecallRequest>(body)))
                    request.path == "/visibility" && request.method == "POST" ->
                        respond(
                            200,
                            store.visibility(Json.decodeFromString<VisibilityRequest>(body).nodeIds),
                        )
                    request.path == "/nodes" && request.method == "POST" ->
                        respond(201, store.save(Json.decodeFromString<NodeDraft>(body)))
                    path.size == 2 && path[0] == "nodes" && request.method == "GET" ->
                        respond(200, store.get(path[1]))
                    path.size == 2 && path[0] == "nodes" && request.method == "PUT" ->
                        respond(200, store.save(Json.decodeFromString<NodeDraft>(body), path[1]))
                    path.size == 2 && path[0] == "nodes" && request.method == "DELETE" -> {
                        store.deleteNode(path[1])
                        empty()
                    }
                    path.size == 3 &&
                        path[0] == "nodes" &&
                        path[2] == "edges" &&
                        request.method == "GET" -> {
                        store.get(path[1])
                        respond(200, store.links(listOf(path[1])))
                    }
                    path.size == 3 &&
                        path[0] == "nodes" &&
                        path[2] == "revisions" &&
                        request.method == "GET" -> respond(200, store.revisions(path[1]))
                    path.size == 3 &&
                        path[0] == "nodes" &&
                        path[2] == "rollback" &&
                        request.method == "POST" ->
                        respond(
                            200,
                            store.rollback(path[1], Json.decodeFromString<RollbackRequest>(body)),
                        )
                    path.size == 3 &&
                        path[0] == "nodes" &&
                        path[2] == "sources" &&
                        request.method == "GET" ->
                        respond(200, store.evidence(path[1]).map(intake::source))
                    request.path == "/edges" && request.method == "POST" ->
                        respond(201, store.saveEdge(Json.decodeFromString<EdgeDraft>(body)))
                    path.size == 2 && path[0] == "edges" && request.method == "DELETE" -> {
                        store.deleteEdge(path[1])
                        empty()
                    }
                    request.path == "/import" && request.method == "POST" -> {
                        val imported = Json.decodeFromString<ImportRequest>(body)
                        val source =
                            intake.capture(
                                CaptureRequest(
                                    imported.requestId,
                                    listOf(imported.title, imported.url, imported.text)
                                        .filter { it.isNotBlank() }
                                        .joinToString("\n"),
                                    space.id,
                                    "import",
                                )
                            )
                        respond(
                            201,
                            ImportResult(store.get(source.id), store.links(listOf(source.id)).size),
                        )
                    }
                    request.path == "/context" && request.method == "POST" ->
                        respond(
                            200,
                            store.exportContext(Json.decodeFromString<ContextRequest>(body)),
                        )
                    else -> respond(404, Failure("接口不存在"))
                }
            }
        } catch (failure: InputFailure) {
            respond(400, Failure(failure.message ?: "输入无效"))
        } catch (failure: AccessDenied) {
            respond(403, Failure("没有访问权限"))
        } catch (failure: SerializationException) {
            respond(400, Failure("请求 JSON 不符合接口契约"))
        } catch (failure: MissingRecord) {
            respond(404, Failure(failure.message!!))
        } catch (failure: StaleRecord) {
            respond(409, Failure(failure.message!!))
        } catch (failure: Throwable) {
            Host.log("Memory 请求失败，存储事务已回滚")
            respond(503, Failure("存储暂不可用，请稍后重试"))
        }
    }

    private inline fun <reified T> respond(status: Int, value: T) =
        Transport.Response(
            status.toUShort(),
            listOf(
                Transport.Header("content-type", "application/json; charset=utf-8"),
                Transport.Header("cache-control", "no-store"),
            ),
            wireJson.encodeToString(value).encodeToByteArray().map { it.toUByte() },
        )

    private fun empty() = Transport.Response(204U.toUShort(), emptyList(), emptyList())
}
