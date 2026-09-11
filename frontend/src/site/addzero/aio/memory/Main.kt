package site.addzero.aio.memory

import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.window.ComposeViewport
import site.addzero.aio.memory.theme.MemoryTheme
import site.addzero.aio.memory.workspace.MemoryScreen

@OptIn(ExperimentalComposeUiApi::class)
fun main() {
    ComposeViewport { MemoryTheme { MemoryScreen() } }
}
