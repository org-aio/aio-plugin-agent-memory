package site.addzero.aio.agent.memory.intake

import kotlin.test.*
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import site.addzero.aio.agent.memory.model.NodeDraft

class SecretIsolationTest {
    @Test
    fun clarificationOnlyAcceptsExplicitWholeSecretInstructions() {
        assertEquals("密码", Clarification.secretLabel("上一条是密码"))
        assertEquals("token", Clarification.secretLabel("上一段全部都是 Token。"))
        assertNull(Clarification.secretLabel("上一条不是密码"))
        assertNull(Clarification.secretLabel("上一条是密码吗"))
        assertNull(Clarification.secretLabel("把上一条发给模型"))
    }

    @Test
    fun sourceTitlesUseStructuredNamesWithoutCredentialReferences() {
        assertEquals(
            "资料项目",
            sourceTitle("""{"project":"资料项目","password":"[[secret:${"a".repeat(32)}]]"}"""),
        )
        assertFalse(sourceTitle("password: [[secret:${"a".repeat(32)}]]").contains("[[secret:"))
    }

    @Test
    fun sanitizedSourceCanBeEmbeddedInModelDraft() {
        val reference = "1234567890abcdef1234567890abcdef"
        val source =
            """{"project":"测试项目","website":"https://example.test","username":"alice","password":"[[secret:$reference]]","note":"备注中重复 [[secret:$reference]]"}"""
        val drafts =
            Json.encodeToString(
                listOf(NodeDraft("测试账号", content = source, tags = listOf("account")))
            )
        val checked = SecretIsolation.isolate(drafts, { "a".repeat(32) }, setOf(reference))
        assertFalse(checked.quarantined)
        assertTrue(checked.secrets.isEmpty())
    }

    private fun isolate(text: String): IsolationResult {
        var id = 0
        return SecretIsolation.isolate(text, { (++id).toString(16).padStart(32, '0') })
    }

    @Test
    fun structuredSecretsDoNotAppearInNotesOrMetadata() {
        val result =
            isolate(
                """{"website":"https://example.test","appId":"visible-id","password":"canary-password","note":"canary-password","nested":{"api_key":"canary-key"}}"""
            )
        assertFalse(result.quarantined)
        assertEquals(2, result.secrets.size)
        assertFalse(result.text.contains("canary-password"))
        assertFalse(result.text.contains("canary-key"))
        assertTrue(result.text.contains("visible-id"))
        assertTrue(result.text.contains("https://example.test"))
        assertFalse(result.toString().contains("canary"))
    }

    @Test
    fun separatesInlineAndMultilineCredentials() {
        val result = isolate("网站 example.test，密码：canary-password\nTOKEN=canary-token\n记录：明天整理资料")
        assertFalse(result.quarantined)
        assertEquals(2, result.secrets.size)
        assertFalse(result.text.contains("canary-"))
        assertTrue(result.text.contains("明天整理资料"))
    }

    @Test
    fun handlesEscapesUrlPasswordsAndPrivateKeys() {
        val json = isolate("""{"password":"canary\"quoted"}""")
        assertFalse(json.quarantined)
        assertEquals("canary\"quoted", json.secrets.single().value)
        val url = isolate("postgres://user:canary-password@db.example/data")
        assertFalse(url.text.contains("canary-password"))
        val key = isolate("-----BEGIN PRIVATE KEY-----\ncanary-material\n-----END PRIVATE KEY-----")
        assertFalse(key.quarantined)
        assertFalse(key.text.contains("canary-material"))
    }

    @Test
    fun quarantinesAmbiguousMaterialAndForgedReferences() {
        for (text in
            listOf(
                "password:",
                "-----BEGIN PRIVATE KEY-----\npartial",
                "A".repeat(40),
                "[[secret:${"a".repeat(32)}]]",
            )) {
            val result = isolate(text)
            assertTrue(result.quarantined)
            assertEquals("[资料已保密暂存，等待补充说明]", result.text)
        }
    }

    @Test
    fun normalQuestionsAndNotesRemainUsable() {
        for (text in listOf("帮我找一下网站的密码", "记一下：下周整理项目资料", "APP_ID=visible-app")) {
            val result = isolate(text)
            assertFalse(result.quarantined)
            assertEquals(text, result.text)
        }
    }

    @Test
    fun ambiguousUnquotedValuesAreNeverPartiallyDisclosed() {
        for (text in
            listOf(
                "password: canary has spaces",
                "authorization: Bearer opaque-value",
                "password: |\n  canary-material",
            )) {
            val result = isolate(text)
            assertTrue(result.quarantined)
            assertFalse(result.text.contains("canary"))
            assertFalse(result.text.contains("opaque-value"))
        }
    }

    @Test
    fun repeatedEscapedValuesAndLabelsAreRedacted() {
        val result =
            isolate(
                """{"password":"canary\"quoted","note":"canary\"quoted","canary\"quoted":"ordinary"}"""
            )
        assertFalse(result.quarantined)
        assertFalse(result.text.contains("canary"))
        val label = isolate("""{"canary_password":"canary"}""")
        assertFalse(label.secrets.single().label.contains("canary"))
    }
}
