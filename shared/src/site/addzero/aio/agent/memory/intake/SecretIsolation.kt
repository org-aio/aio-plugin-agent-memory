package site.addzero.aio.agent.memory.intake

import kotlinx.serialization.json.*

// 秘密类型故意不实现序列化，避免被公共响应或日志直接编码。
class IsolatedSecret(val id: String, val label: String, val value: String) {
    override fun toString() = "IsolatedSecret([protected])"
}

class IsolationResult(
    val text: String,
    val secrets: List<IsolatedSecret>,
    val quarantined: Boolean,
) {
    override fun toString() = "IsolationResult([protected])"
}

object SecretIsolation {
    private val names =
        setOf(
            "password",
            "passwd",
            "pwd",
            "passphrase",
            "token",
            "accesstoken",
            "refreshtoken",
            "apikey",
            "secret",
            "clientsecret",
            "appsecret",
            "secretkey",
            "privatekey",
            "authorization",
            "cookie",
            "密码",
            "口令",
            "密钥",
            "秘钥",
            "令牌",
        )
    private val assignment by lazy {
        Regex(
            "(password|passwd|pwd|passphrase|(?:access[_ -]?|refresh[_ -]?)?token|api[_ -]?key|(?:client[_ -]?|app[_ -]?)?secret|secret[_ -]?key|private[_ -]?key|authorization|cookie|密码|口令|密钥|秘钥|令牌)\\s*[:=：是为]\\s*(\"(?:[^\"\\\\]|\\\\.)*\"|'[^']*'|[^\\s,;，；]+)"
        )
    }
    private val incompleteAssignment by lazy {
        Regex("(password|passwd|pwd|token|secret|api[_ -]?key|密码|密钥|秘钥|令牌)\\s*[:=：是为]")
    }
    private val privateKey by lazy {
        Regex("-----BEGIN [A-Z ]*PRIVATE KEY-----[\\s\\S]*?-----END [A-Z ]*PRIVATE KEY-----")
    }
    private val knownToken by lazy {
        Regex(
            "(?:sk-[A-Za-z0-9_-]{16,}|gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|eyJ[A-Za-z0-9_-]+\\.[A-Za-z0-9_-]+\\.[A-Za-z0-9_-]+)"
        )
    }
    private val bearer by lazy { Regex("\\bbearer\\s+([A-Za-z0-9._~+/-]+=*)") }
    private val userInfo by lazy { Regex("([a-zA-Z][a-zA-Z0-9+.-]*://[^\\s/@:]+:)([^\\s/@]+)(@)") }
    private val suspiciousOpaque by lazy { Regex("[A-Za-z0-9_+/=-]{32,}") }
    private val secretReference by lazy { Regex("\\[\\[secret:[a-f0-9]{32}]]") }

    // 只折叠 ASCII 字段名，保持 UTF-16 索引和原始凭据大小写不变。
    private fun asciiLower(value: String) =
        value.map { if (it in 'A'..'Z') it + 32 else it }.joinToString("")

    private fun replaceNamed(
        input: String,
        pattern: Regex,
        transform: (MatchResult, String) -> String,
    ): String = buildString {
        var position = 0
        val references = secretReference.findAll(input).map { it.range }.toList()
        pattern.findAll(asciiLower(input)).forEach { match ->
            if (references.any { match.range.first in it }) return@forEach
            append(input.substring(position, match.range.first))
            append(transform(match, input.substring(match.range)))
            position = match.range.last + 1
        }
        append(input.substring(position))
    }

    fun isolate(
        input: String,
        nextId: () -> String,
        allowedReferences: Set<String> = emptySet(),
    ): IsolationResult {
        val secrets = mutableListOf<IsolatedSecret>()
        var checkedInput = input
        allowedReferences.forEach {
            checkedInput = checkedInput.replace("[[secret:$it]]", "[protected]")
        }
        var uncertain = checkedInput.contains("[[secret:")
        fun protect(label: String, value: String): String {
            if (allowedReferences.any { value == "[[secret:$it]]" }) return value
            if (value.isBlank()) {
                uncertain = true
                return "[protected]"
            }
            val existing = secrets.firstOrNull { it.value == value }
            val normalized = label.lowercase().filter { it.isLetterOrDigit() }
            val safeLabel =
                names.sortedByDescending { it.length }.firstOrNull { normalized.endsWith(it) }
                    ?: "secret"
            val secret = existing ?: IsolatedSecret(nextId(), safeLabel, value).also(secrets::add)
            return "[[secret:${secret.id}]]"
        }
        fun plain(value: String): String {
            var result = privateKey.replace(value) { protect("private_key", it.value) }
            if (result.contains("-----BEGIN") && result.contains("PRIVATE KEY")) uncertain = true
            val assignmentInput = result
            result =
                replaceNamed(assignmentInput, assignment) { match, original ->
                    val raw = original.takeLast(match.groupValues[2].length)
                    val tail =
                        assignmentInput
                            .substring(match.range.last + 1)
                            .substringBefore('\n')
                            .substringBefore(',')
                            .substringBefore(';')
                            .substringBefore('，')
                            .substringBefore('；')
                            .trim()
                    if (
                        !raw.startsWith('"') &&
                            !raw.startsWith('\'') &&
                            tail.isNotEmpty() &&
                            !assignment.containsMatchIn(asciiLower(tail))
                    )
                        uncertain = true
                    val decoded =
                        if (raw.startsWith('"'))
                            runCatching { Json.parseToJsonElement(raw).jsonPrimitive.content }
                                .getOrElse {
                                    uncertain = true
                                    raw
                                }
                        else if (raw.startsWith('\'')) raw.drop(1).dropLast(1) else raw
                    if (
                        decoded in setOf("|", ">", "|-", "|+", ">-", ">+") ||
                            decoded.startsWith("!!")
                    )
                        uncertain = true
                    "${match.groupValues[1]}: ${protect(match.groupValues[1], decoded)}"
                }
            result =
                replaceNamed(result, bearer) { match, original ->
                    "Bearer ${protect("authorization", original.takeLast(match.groupValues[1].length))}"
                }
            result =
                userInfo.replace(result) {
                    "${it.groupValues[1]}${protect("url_password", it.groupValues[2])}${it.groupValues[3]}"
                }
            result = knownToken.replace(result) { protect("token", it.value) }
            val withoutReferences = result.replace(secretReference, "[protected]")
            if (withoutReferences.lineSequence().any { suspiciousOpaque.matches(it.trim()) })
                uncertain = true
            val remaining =
                replaceNamed(withoutReferences, incompleteAssignment) { match, original ->
                    val suffix = withoutReferences.substring(match.range.last + 1).trimStart()
                    if (!suffix.startsWith("[protected]")) uncertain = true
                    original
                }
            if (remaining.contains("-----BEGIN")) uncertain = true
            return result
        }
        fun sensitive(name: String): Boolean {
            val normalized = name.lowercase().filter { it.isLetterOrDigit() }
            return normalized in names ||
                names.filter { it.length >= 5 }.any { normalized.endsWith(it) }
        }
        fun visit(element: JsonElement): JsonElement =
            when (element) {
                is JsonObject ->
                    JsonObject(
                        element
                            .map { (key, value) ->
                                plain(key) to
                                    if (sensitive(key)) {
                                        val raw =
                                            if (value is JsonPrimitive && value.isString)
                                                value.content
                                            else value.toString()
                                        JsonPrimitive(protect(key, raw))
                                    } else visit(value)
                            }
                            .toMap()
                    )
                is JsonArray -> JsonArray(element.map(::visit))
                is JsonPrimitive ->
                    if (element.isString) JsonPrimitive(plain(element.content)) else element
            }
        val trimmed = input.trim()
        val json =
            if (trimmed.startsWith('{') || trimmed.startsWith('['))
                runCatching { Json.parseToJsonElement(trimmed) }.getOrNull()
            else null
        val structured = json?.let(::visit)
        val initial = structured?.toString() ?: plain(input)
        if (
            json == null &&
                (trimmed.startsWith('{') || trimmed.startsWith('[')) &&
                incompleteAssignment.containsMatchIn(asciiLower(input))
        )
            uncertain = true
        // 同一个值出现在别名或备注中也必须替换；保留已生成的引用，避免再次解析引用内部。
        fun redact(value: String): String {
            var safe = value
            secrets
                .sortedByDescending { it.value.length }
                .forEach { secret ->
                    val pieces = mutableListOf<String>()
                    var position = 0
                    secretReference.findAll(safe).forEach { match ->
                        pieces +=
                            safe
                                .substring(position, match.range.first)
                                .replace(secret.value, "[[secret:${secret.id}]]")
                        pieces += match.value
                        position = match.range.last + 1
                    }
                    pieces +=
                        safe.substring(position).replace(secret.value, "[[secret:${secret.id}]]")
                    safe = pieces.joinToString("")
                }
            return safe
        }
        fun redactJson(element: JsonElement): JsonElement =
            when (element) {
                is JsonObject ->
                    JsonObject(
                        element.map { (key, value) -> redact(key) to redactJson(value) }.toMap()
                    )
                is JsonArray -> JsonArray(element.map(::redactJson))
                is JsonPrimitive ->
                    if (element.isString) JsonPrimitive(redact(element.content)) else element
            }
        val safe = structured?.let(::redactJson)?.toString() ?: redact(initial)
        return IsolationResult(if (uncertain) "[资料已保密暂存，等待补充说明]" else safe, secrets, uncertain)
    }
}
