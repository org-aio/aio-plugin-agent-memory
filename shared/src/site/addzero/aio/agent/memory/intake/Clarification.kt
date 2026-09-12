package site.addzero.aio.agent.memory.intake

object Clarification {
    private val wholeSecret by lazy {
        Regex(
            "(?:上一条|上一段|刚才那段|上面那段)(?:内容|资料)?(?:全部|整个|都是|是|为|就是|都属于)+\\s*(密码|口令|密钥|秘钥|令牌|token|password|api key|private key)[。.!！]?"
        )
    }

    fun secretLabel(explanation: String): String? {
        if (explanation.length > 100) return null
        return wholeSecret.matchEntire(explanation.trim().lowercase())?.groupValues?.get(1)
    }
}
