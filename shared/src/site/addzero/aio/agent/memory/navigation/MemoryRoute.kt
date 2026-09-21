package site.addzero.aio.agent.memory.navigation

import site.addzero.aio.agent.memory.model.NodeKind

/**
 * 页面状态在 URL 片段中的表示。
 *
 * 片段是唯一真相：界面从它派生，用户操作写回它，再由它驱动渲染。
 * 编码结果必须幂等，同一状态只对应一条片段。
 */
data class MemoryRoute(
    val view: String = WIKI,
    val nodeId: String? = null,
    val spaceId: String? = null,
    val query: String = "",
    val kind: NodeKind? = null,
) {
    /** 规范片段，含前导 `#`；无状态时返回空串。 */
    fun encode(): String {
        val parts = mutableListOf<String>()
        if (view != WIKI) parts += "view=" + Percent.encode(view)
        nodeId?.takeIf { it.isNotBlank() }?.let { parts += "node=" + Percent.encode(it) }
        spaceId?.takeIf { it.isNotBlank() }?.let { parts += "space=" + Percent.encode(it) }
        if (query.isNotBlank()) parts += "q=" + Percent.encode(query)
        kind?.let { parts += "kind=" + it.name }
        return if (parts.isEmpty()) "" else "#" + parts.joinToString("&")
    }

    companion object {
        const val WIKI = "wiki"
        val VIEWS = listOf(WIKI, "graph", "sources", "secrets", "pending")

        /** 解析片段；缺失或非法取值回落到默认值，不抛错。 */
        fun parse(fragment: String): MemoryRoute {
            val fields =
                fragment.removePrefix("#").split("&")
                    .mapNotNull { item ->
                        val index = item.indexOf('=')
                        if (index <= 0) null else item.take(index) to Percent.decode(item.drop(index + 1))
                    }
                    .toMap()
            val view = fields["view"]?.takeIf { it in VIEWS } ?: WIKI
            val nodeId = fields["node"]?.takeIf { it.isNotBlank() && it.length <= 64 }
            val spaceId = fields["space"]?.takeIf { it.isNotBlank() && it.length <= 64 }
            val query = fields["q"]?.take(256) ?: ""
            val kind = fields["kind"]?.let { name -> NodeKind.entries.firstOrNull { it.name == name } }
            return MemoryRoute(view, nodeId, spaceId, query, kind)
        }
    }
}

/** 片段参数的最小百分号编解码，只处理 UTF-8，不依赖平台库。 */
private object Percent {
    private val hex = "0123456789ABCDEF"

    fun encode(value: String): String = buildString {
        for (byte in value.encodeToByteArray()) {
            val code = byte.toInt() and 0xFF
            val ch = code.toChar()
            if (ch.isLetterOrDigit() && code < 0x80 || ch in "-_.~") {
                append(ch)
            } else {
                append('%').append(hex[code shr 4]).append(hex[code and 0x0F])
            }
        }
    }

    fun decode(value: String): String? {
        val bytes = ArrayList<Byte>(value.length)
        var index = 0
        while (index < value.length) {
            val ch = value[index]
            if (ch == '%') {
                if (index + 2 >= value.length) return null
                val high = value[index + 1].digitToIntOrNull(16) ?: return null
                val low = value[index + 2].digitToIntOrNull(16) ?: return null
                bytes += ((high shl 4) or low).toByte()
                index += 3
            } else {
                if (ch.code > 0x7F) return null
                bytes += ch.code.toByte()
                index += 1
            }
        }
        return decodeUtf8(bytes.toByteArray())
    }

    /** 严格 UTF-8 校验；非法字节序列返回 null，避免把乱码当成有效参数。 */
    private fun decodeUtf8(bytes: ByteArray): String? {
        val text = StringBuilder(bytes.size)
        var index = 0
        while (index < bytes.size) {
            val first = bytes[index].toInt() and 0xFF
            val extra =
                when {
                    first < 0x80 -> 0
                    first in 0xC2..0xDF -> 1
                    first in 0xE0..0xEF -> 2
                    first in 0xF0..0xF4 -> 3
                    else -> return null
                }
            if (index + extra >= bytes.size) return null
            var code = first and (0x7F shr extra)
            for (offset in 1..extra) {
                val next = bytes[index + offset].toInt() and 0xFF
                if (next !in 0x80..0xBF) return null
                code = (code shl 6) or (next and 0x3F)
            }
            if (code > 0x10FFFF || code in 0xD800..0xDFFF) return null
            if (code < 0x10000) {
                text.append(code.toChar())
            } else {
                // 补充平面字符拆成 UTF-16 代理对。
                val offset = code - 0x10000
                text.append((0xD800 + (offset shr 10)).toChar())
                text.append((0xDC00 + (offset and 0x3FF)).toChar())
            }
            index += extra + 1
        }
        return text.toString()
    }
}
