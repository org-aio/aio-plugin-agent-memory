package site.addzero.aio.agent.memory.transport

import kotlinx.serialization.SerializationException
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import site.addzero.aio.agent.memory.bindings.PluginRootFunctions
import site.addzero.aio.agent.memory.bindings.*
import site.addzero.aio.agent.memory.model.*
import site.addzero.aio.agent.memory.storage.MemoryStore
import site.addzero.aio.agent.memory.retrieval.exportContext

internal object PluginRootFunctionsExportsImpl : PluginRootFunctions.Exports {
    override fun describe() = Metadata.Description("Agent Memory", listOf(Metadata.PageDefinition(
        "agent-memory", "记忆图谱", "index.html", Metadata.Scene("community", "社区插件"), listOf("Agent"), null, Metadata.Surface.WORKSPACE,
    )))
    // 健康检查阶段不开放宿主能力；连接和迁移由宿主验证，业务访问在请求事务中执行。
    override fun health(): Result<Unit> = Result.success(Unit)
    override fun lifecycle(phase: Transport.Phase): Result<Unit> = Result.success(Unit)

    override fun handle(request: Transport.Request): Transport.Response {
        val context = Host.context()
        if (context.tenantId.isNullOrBlank() || context.userId.isNullOrBlank()) return respond(401, Failure("请先登录"))
        if (request.body.size > 512_000) return respond(413, Failure("请求正文过大"))
        return try {
            MemoryStore.transaction { store ->
                val path = request.path.split('/').filter(String::isNotEmpty)
                val body = request.body.map { it.toByte() }.toByteArray().decodeToString()
                when {
                    request.path == "/graph" && request.method == "GET" -> respond(200, store.graph(SearchRequest()))
                    request.path == "/search" && request.method == "POST" -> respond(200, store.graph(Json.decodeFromString<SearchRequest>(body)))
                    request.path == "/nodes" && request.method == "POST" -> respond(201, store.save(Json.decodeFromString<NodeDraft>(body)))
                    path.size == 2 && path[0] == "nodes" && request.method == "GET" -> respond(200, store.get(path[1]))
                    path.size == 2 && path[0] == "nodes" && request.method == "PUT" -> respond(200, store.save(Json.decodeFromString<NodeDraft>(body), path[1]))
                    path.size == 2 && path[0] == "nodes" && request.method == "DELETE" -> { store.deleteNode(path[1]); empty() }
                    path.size == 3 && path[0] == "nodes" && path[2] == "edges" && request.method == "GET" -> { store.get(path[1]); respond(200, store.links(listOf(path[1]))) }
                    request.path == "/edges" && request.method == "POST" -> respond(201, store.saveEdge(Json.decodeFromString<EdgeDraft>(body)))
                    path.size == 2 && path[0] == "edges" && request.method == "DELETE" -> { store.deleteEdge(path[1]); empty() }
                    request.path == "/import" && request.method == "POST" -> respond(201, store.importSource(Json.decodeFromString<ImportRequest>(body)))
                    request.path == "/context" && request.method == "POST" -> respond(200, store.exportContext(Json.decodeFromString<ContextRequest>(body)))
                    else -> respond(404, Failure("接口不存在"))
                }
            }
        } catch (failure: InputFailure) { respond(400, Failure(failure.message ?: "输入无效"))
        } catch (failure: SerializationException) { respond(400, Failure("请求 JSON 不符合接口契约"))
        } catch (failure: MissingRecord) { respond(404, Failure(failure.message!!))
        } catch (failure: StaleRecord) { respond(409, Failure(failure.message!!))
        } catch (failure: Throwable) {
            Host.log("Memory 请求失败: ${failure.message}")
            respond(503, Failure("存储暂不可用，请稍后重试"))
        }
    }
    private inline fun <reified T> respond(status: Int, value: T) = Transport.Response(
        status.toUShort(), listOf(Transport.Header("content-type", "application/json; charset=utf-8")),
        Json.encodeToString(value).encodeToByteArray().map { it.toUByte() },
    )
    private fun empty() = Transport.Response(204U.toUShort(), emptyList(), emptyList())
}
