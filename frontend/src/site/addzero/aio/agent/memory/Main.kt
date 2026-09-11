package site.addzero.aio.agent.memory

import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.window.ComposeViewport
import site.addzero.aio.agent.memory.theme.MemoryTheme
import site.addzero.aio.agent.memory.workspace.MemoryScreen

@OptIn(ExperimentalComposeUiApi::class)
fun main() {
    ComposeViewport { MemoryTheme { MemoryScreen() } }
}
