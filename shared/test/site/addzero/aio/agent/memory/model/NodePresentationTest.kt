package site.addzero.aio.agent.memory.model

import kotlin.test.*

class NodePresentationTest {
    private fun node(kind: NodeKind, content: String = "正文", tags: List<String> = emptyList(), url: String = "") =
        MemoryNode("id", "标题", kind, content, url, tags)

    private fun labels(node: MemoryNode) = node.fields().map { it.label }

    @Test fun everyKindStartsWithTitle() {
        NodeKind.entries.forEach { assertEquals("标题", labels(node(it)).first()) }
    }

    @Test fun kindsCarryDistinctFieldNames() {
        assertEquals(listOf("标题", "正文"), labels(node(NodeKind.NOTE)))
        assertEquals(listOf("标题", "定义"), labels(node(NodeKind.CONCEPT)))
        assertEquals(listOf("标题", "简介"), labels(node(NodeKind.PERSON)))
        assertEquals(listOf("标题", "经过"), labels(node(NodeKind.EVENT)))
        assertEquals(listOf("标题", "范围"), labels(node(NodeKind.PROJECT)))
        assertEquals(listOf("标题", "摘要"), labels(node(NodeKind.SOURCE)))
    }

    @Test fun optionalFieldsAppearOnlyWhenPresent() {
        assertFalse(labels(node(NodeKind.NOTE)).contains("标签"))
        assertTrue(labels(node(NodeKind.NOTE, tags = listOf("KMP"))).contains("标签"))
        assertFalse(labels(node(NodeKind.NOTE)).contains("来源地址"))
        assertTrue(labels(node(NodeKind.NOTE, url = "https://example.com")).contains("来源地址"))
    }

    @Test fun aliasesRenderWhenPresent() {
        val withAliases = node(NodeKind.CONCEPT).copy(aliases = listOf("图", "关系图"))
        assertEquals("图、关系图", withAliases.fields().first { it.label == "别名" }.value)
    }

    @Test fun blankContentFallsBackToPlaceholder() {
        val empty = node(NodeKind.NOTE, content = "")
        assertEquals("暂无正文", empty.fields().first { it.label == "正文" }.value)
    }
}
