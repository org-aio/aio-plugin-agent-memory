package site.addzero.aio.agent.memory.model

/**
 * 详情面板中的一个字段。
 *
 * [label] 是面向用户的字段名，[value] 是已格式化的展示值。平台无关，便于测试，
 * 也避免界面层为每种节点类型各写一套分支。
 */
data class NodeField(val label: String, val value: String, val multiline: Boolean = false)

/**
 * 按节点类型给出详情字段。
 *
 * 不同类型强调不同信息：笔记重正文，概念重定义与别名，人物重身份与关联，
 * 事件重时间与来源，项目重范围与出处，来源重出处与凭据。
 */
fun MemoryNode.fields(): List<NodeField> = buildList {
    add(NodeField("标题", title))
    if (aliases.isNotEmpty()) add(NodeField("别名", aliases.joinToString("、")))
    when (kind) {
        NodeKind.NOTE -> {
            add(NodeField("正文", content.ifBlank { "暂无正文" }, multiline = true))
        }
        NodeKind.CONCEPT -> add(NodeField("定义", content.ifBlank { "暂无定义" }, multiline = true))
        NodeKind.PERSON -> add(NodeField("简介", content.ifBlank { "暂无简介" }, multiline = true))
        NodeKind.EVENT -> add(NodeField("经过", content.ifBlank { "暂无记录" }, multiline = true))
        NodeKind.PROJECT -> add(NodeField("范围", content.ifBlank { "暂无说明" }, multiline = true))
        NodeKind.SOURCE -> add(NodeField("摘要", content.ifBlank { "暂无摘要" }, multiline = true))
    }
    if (tags.isNotEmpty()) add(NodeField("标签", tags.joinToString("、") { "#$it" }))
    if (url.isNotBlank()) add(NodeField("来源地址", url))
}
