package site.addzero.aio.agent.memory.routing

import kotlin.test.*

class ChatClassifierTest {
    @Test
    fun explicitLookupExtractsSubject() {
        for ((input, expected) in
            listOf(
                "查找 极光项目" to "极光项目",
                "极光密码是多少？" to "极光密码",
                "会议安排是什么？" to "会议安排",
                "find Aurora" to "Aurora",
            )) {
            assertEquals(
                RoutingDecision(ChatIntent.RECALL, expected),
                ChatClassifier.classify(input),
            )
        }
    }

    @Test
    fun mixedAndUncertainRequestsUseModel() {
        for (input in
            listOf(
                "比较极光和北辰项目",
                "搜索极光，然后分析风险",
                "查找极光并且解释原因",
                "查找极光；保存新的安排",
                "find Aurora and Borealis",
                "继续",
                "我应该怎么做？",
            )) {
            assertEquals(ChatIntent.MODEL, ChatClassifier.classify(input).intent, input)
        }
    }

    @Test
    fun savesAreLocalButRequestsForAnalysisRemainModelWork() {
        val ref = "[[secret:${"a".repeat(32)}]]"
        for (input in
            listOf(
                "记下：下周三开会",
                "会议定在周五，请记下来",
                "password: $ref",
                """{"project":"Aurora","password":"$ref"}""",
            )) {
            assertEquals(ChatIntent.SAVE, ChatClassifier.classify(input).intent, input)
        }
        assertEquals(ChatIntent.MODEL, ChatClassifier.classify("password: $ref\n分析项目风险").intent)
    }
}
