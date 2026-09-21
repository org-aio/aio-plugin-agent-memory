package site.addzero.aio.agent.memory.navigation

import kotlin.test.*
import site.addzero.aio.agent.memory.model.NodeKind

class MemoryRouteTest {
    @Test fun emptyStateHasNoFragment() {
        assertEquals("", MemoryRoute().encode())
        assertEquals(MemoryRoute(), MemoryRoute.parse(""))
    }

    @Test fun roundTripsEveryField() {
        val route = MemoryRoute("graph", "n1", "s2", "玻璃 工艺", NodeKind.NOTE)
        assertEquals(route, MemoryRoute.parse(route.encode()))
    }

    @Test fun encodingIsCanonical() {
        val once = MemoryRoute("sources", "a/b", null, "q=1&x", null).encode()
        assertEquals(once, MemoryRoute.parse(once).encode())
        assertEquals("#view=sources&node=a%2Fb&q=q%3D1%26x", once)
    }

    @Test fun fallsBackOnInvalidInput() {
        assertEquals(MemoryRoute(), MemoryRoute.parse("#view=unknown"))
        assertEquals(MemoryRoute(), MemoryRoute.parse("#node="))
        assertEquals(MemoryRoute(), MemoryRoute.parse("#kind=NOPE"))
        assertEquals(MemoryRoute(), MemoryRoute.parse("#node=%E0%A4"))
    }
}
