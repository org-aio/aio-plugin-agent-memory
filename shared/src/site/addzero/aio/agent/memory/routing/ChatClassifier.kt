package site.addzero.aio.agent.memory.routing

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject

object ChatClassifier {
    private val complex by lazy {
        Regex(
            "分析|比较|对比|总结|归纳|解释|为什么|为何|怎么|如何|建议|规划|设计|推理|翻译|生成|撰写|编写|" +
                "\\b(analy[sz]e|compare|summari[sz]e|explain|why|how|recommend|plan|write|translate)\\b"
        )
    }
    private val lookup by lazy {
        Regex(
            "^(?:请)?(?:帮我)?(?:查找|搜索|检索|查一下|查下|找一下|找下|找出|查询|查阅|查看)\\s*[:：]?\\s*(.+)$|" +
                "^(?:find|search(?: for)?|look up|lookup|show)\\s+(.+)$"
        )
    }
    private val fact by lazy { Regex("^(.{2,80}?)(?:是什么|是啥|在哪里|在哪儿|在哪|是多少|是哪天|是什么时候)[？?。!！]*$") }
    private val save by lazy {
        Regex(
            "^(?:请)?(?:帮我)?(?:记下|记住|记录|保存|备忘|收下)(?:来)?[\\s:：，,][\\s\\S]+|" +
                "[\\s\\S]+[，,。\\s](?:请)?(?:帮我)?(?:记下|记住|保存|记录)(?:来)?[。!！]*$|" +
                "^(?:remember|save|note)\\s+[\\s\\S]+"
        )
    }
    private val secretReference by lazy { Regex("\\[\\[secret:[a-f0-9]{32}]]") }

    fun classify(sanitized: String): RoutingDecision {
        val text = sanitized.trim()
        val question = text.replace(secretReference, "").trim()
        val comparable = asciiLower(question)
        if (complex.containsMatchIn(comparable)) return RoutingDecision(ChatIntent.MODEL, question)
        // 只有单句、无秘密赋值的显式查找才能跳过模型；复合输入保留模型回退。
        if (
            !secretReference.containsMatchIn(text) &&
                !question.contains('\n') &&
                question.none { it in "；;" } &&
                question.length <= 180
        ) {
            val match = lookup.matchEntire(comparable) ?: fact.matchEntire(comparable)
            val subject =
                match
                    ?.groups
                    ?.drop(1)
                    ?.filterNotNull()
                    ?.firstOrNull()
                    ?.range
                    ?.let { question.substring(it) }
                    ?.trim()
                    ?.trimEnd('?', '？', '。', '!', '！')
            if (
                !subject.isNullOrBlank() &&
                    subject.none { it in "，,。？?" } &&
                    !Regex("然后|并且|顺便|以及|\\band\\b").containsMatchIn(asciiLower(subject))
            )
                return RoutingDecision(ChatIntent.RECALL, subject)
        }
        val structured =
            if (text.startsWith('{') || text.startsWith('['))
                runCatching { Json.parseToJsonElement(text) }.getOrNull()
            else null
        if (
            save.matches(asciiLower(text)) ||
                structured is JsonObject ||
                structured is JsonArray ||
                (secretReference.containsMatchIn(text) && question.none { it in "？?" })
        )
            return RoutingDecision(ChatIntent.SAVE, question)
        return RoutingDecision(ChatIntent.MODEL, question)
    }

    private fun asciiLower(value: String) =
        value.map { if (it in 'A'..'Z') it + 32 else it }.joinToString("")
}
