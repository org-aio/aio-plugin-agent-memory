package site.addzero.aio.agent.memory.graph

import androidx.compose.ui.geometry.Offset
import site.addzero.component.knowledge_graph.NodeState
import site.addzero.component.knowledge_graph.layout.ForceDirectedLayout
import site.addzero.component.knowledge_graph.model.*
import kotlin.test.*
import kotlin.math.cos
import kotlin.math.sin

class GraphLayoutTest {
    private fun state(id: String, x: Float) = NodeState(GraphNode(id, id, NodeCategory.DEFAULT, null, null), Offset(x, 100f))
    @Test fun edgesAttractTheirEndpoints() {
        val states = mapOf("a" to state("a", 80f), "b" to state("b", 300f))
        ForceDirectedLayout(500f, 300f, repulsionStrength = 0f, centerGravity = 0f)
            .stepWithStates(states, listOf(GraphEdge("a", "b")))
        assertTrue(states.getValue("a").position.x > 80f)
        assertTrue(states.getValue("b").position.x < 300f)
    }
    @Test fun tinyViewportAndDraggedNodeRemainValid() {
        val node = state("a", 20f)
        val layout = ForceDirectedLayout(20f, 10f)
        layout.stepWithStates(mapOf("a" to node))
        assertTrue(node.position.x.isFinite() && node.position.y.isFinite())
        node.isDragging = true
        val before = node.position
        layout.stepWithStates(mapOf("a" to node))
        assertEquals(before, node.position)
    }
    @Test fun denseGraphKeepsHighlightedNodesApart() {
        val states = (0 until 20).associate { index ->
            val angle = index * 2f * kotlin.math.PI.toFloat() / 20
            val id = index.toString()
            id to NodeState(GraphNode(id, id, NodeCategory.DEFAULT, null, null),
                Offset(500f + cos(angle) * 320f, 210f + sin(angle) * 130f))
        }
        val edges = (0 until 19).map { GraphEdge(it.toString(), (it + 1).toString()) }
        val layout = ForceDirectedLayout(1000f, 420f)
        repeat(360) { layout.stepWithStates(states, edges) }
        val nodes = states.values.toList()
        for (i in nodes.indices) for (j in i + 1 until nodes.size) {
            assertTrue((nodes[i].position - nodes[j].position).getDistance() >= 70f)
        }
        assertTrue(nodes.all { it.position.x in 25f..975f && it.position.y in 25f..395f })
    }
}
