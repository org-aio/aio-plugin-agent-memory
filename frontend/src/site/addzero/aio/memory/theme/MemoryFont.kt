@file:OptIn(kotlin.js.ExperimentalWasmJsInterop::class, kotlin.wasm.unsafe.UnsafeWasmMemoryApi::class)

package site.addzero.aio.memory.theme

import androidx.compose.runtime.*
import androidx.compose.ui.platform.LocalFontFamilyResolver
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.platform.Font
import kotlinx.coroutines.await
import kotlinx.coroutines.CancellationException
import kotlin.js.*
import kotlin.wasm.unsafe.withScopedMemoryAllocator

private fun fetchFont(): Promise<JsAny> = js("fetch(new URL('memory-font.otf', document.baseURI)).then(response => { if (!response.ok) throw new Error('Font unavailable'); return response.arrayBuffer(); }).then(buffer => new Uint8Array(buffer))")
private fun length(bytes: JsAny): Int = js("bytes.length")
private fun copy(bytes: JsAny, address: Int, size: Int): Unit = js("new Uint8Array(wasmExports.memory.buffer, address, size).set(bytes)")

internal class MemoryFontState {
    var font by mutableStateOf<FontFamily?>(null)
    var failed by mutableStateOf(false)
    var attempt by mutableStateOf(0)
}

@Composable
internal fun rememberMemoryFont(): MemoryFontState {
    val resolver = LocalFontFamilyResolver.current
    val state = remember { MemoryFontState() }
    LaunchedEffect(resolver, state.attempt) {
        state.failed = false
        try {
            val bytes = fetchFont().await<JsAny>()
            val data = withScopedMemoryAllocator { allocator ->
                val pointer = allocator.allocate(length(bytes))
                copy(bytes, pointer.address.toInt(), length(bytes))
                ByteArray(length(bytes)) { (pointer + it).loadByte() }
            }
            val family = FontFamily(Font(identity = "memory-noto-cjk", data = data))
            resolver.preload(family)
            state.font = family
        } catch (failure: CancellationException) {
            throw failure
        } catch (_: Throwable) {
            state.failed = true
        }
    }
    return state
}
