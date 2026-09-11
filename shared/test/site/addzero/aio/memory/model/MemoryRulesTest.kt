package site.addzero.aio.memory.model

import kotlin.test.*

class MemoryRulesTest {
    @Test fun normalizesTitlesAndTags() {
        val node = NodeDraft("  项目笔记  ", tags = listOf("KMP", " KMP ", "记忆")).validated()
        assertEquals("项目笔记", node.title)
        assertEquals(listOf("KMP", "记忆"), node.tags)
    }
    @Test fun rejectsInvalidInputs() {
        assertFailsWith<InputFailure> { NodeDraft("").validated() }
        assertFailsWith<InputFailure> { NodeDraft("x", url = "javascript:alert(1)").validated() }
        assertFailsWith<InputFailure> { NodeDraft("x", content = "a".repeat(100001)).validated() }
        assertFailsWith<InputFailure> { EdgeDraft("a".repeat(32), "a".repeat(32), "相关").validated() }
        assertFailsWith<InputFailure> { requireId("../source") }
    }
    @Test fun extractsExplicitWikiLinksOnly() {
        assertEquals(listOf("Compose", "图谱"), wikiLinks("[[Compose]] [[ 图谱 |图]] [[Compose]] [not a link](url)"))
        assertTrue(wikiLinks("ordinary text").isEmpty())
        assertEquals(40, wikiLinks((1..60).joinToString { "[[node$it]]" }).size)
    }
    @Test fun boundsCyclesAndNeighborhood() {
        val edges = listOf(MemoryEdge("1", "a", "b", "r"), MemoryEdge("2", "b", "c", "r"), MemoryEdge("3", "c", "a", "r"))
        assertEquals(setOf("a"), neighborhood(setOf("a"), edges, 0))
        assertEquals(setOf("a", "b", "c"), neighborhood(setOf("a"), edges, 3))
        assertEquals(2, neighborhood(setOf("a"), edges, 3, 2).size)
    }
}
