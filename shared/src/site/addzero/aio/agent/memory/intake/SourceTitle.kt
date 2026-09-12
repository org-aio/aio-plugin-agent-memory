package site.addzero.aio.agent.memory.intake

import kotlinx.serialization.json.*

fun sourceTitle(text: String): String {
    val structured = runCatching { Json.parseToJsonElement(text) as? JsonObject }.getOrNull()
    val named =
        listOf("title", "project", "website", "name", "项目", "标题", "网站").firstNotNullOfOrNull { key
            ->
            (structured?.get(key) as? JsonPrimitive)?.contentOrNull?.takeIf {
                it.isNotBlank() && !it.contains("[[secret:")
            }
        }
    val line =
        named
            ?: if (structured != null) "账号与资料"
            else text.lineSequence().firstOrNull { it.isNotBlank() }.orEmpty()
    return line.replace(Regex("\\[\\[secret:[a-f0-9]{32}]]"), "[保密字段]").take(100).ifBlank { "新资料" }
}
