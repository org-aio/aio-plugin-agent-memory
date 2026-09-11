package site.addzero.aio.agent.memory.graph

import androidx.compose.ui.geometry.Offset
import site.addzero.component.knowledge_graph.NodeState
import site.addzero.component.knowledge_graph.layout.ForceDirectedLayout
import site.addzero.component.knowledge_graph.model.*
import kotlin.test.*

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
}
