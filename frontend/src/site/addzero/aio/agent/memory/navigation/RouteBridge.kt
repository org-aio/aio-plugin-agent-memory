@file:OptIn(kotlin.js.ExperimentalWasmJsInterop::class)

package site.addzero.aio.agent.memory.navigation

import kotlin.js.*

// 与宿主 URL 片段互通。
//
// 插件跑在隔离 iframe 里，外层地址由宿主掌控。宿主注入的桥接脚本监听本框架的
// `hashchange` 并把片段记到宿主路由表，重新挂载时再写回入口 hash。因此这里只改
// 自己的片段，不碰外层地址。Kotlin/Wasm 要求 js(...) 是顶层单表达式，故拆分。

private fun readHash(): JsString = js("location.hash")
// 单表达式：仅在片段变化时用 replaceState 写回，再派发 hashchange 通知宿主。
private fun writeHash(value: JsString): Boolean =
    js("(location.hash === value) || (history.replaceState(null, '', value || location.pathname + location.search), dispatchEvent(new Event('hashchange')), true)")
private fun onHashChange(callback: () -> Unit): Unit =
    js("addEventListener('hashchange', callback)")

internal object RouteBridge {
    /** 当前片段，含前导 `#`；无片段返回空串。 */
    fun read(): String = readHash().toString()

    /**
     * 写入片段并通知宿主。
     *
     * 用 `replaceState` 避免在 iframe 内堆积历史条目，再派发一次 `hashchange`，
     * 让宿主桥接照常收到导航事件。
     */
    fun write(fragment: String) {
        writeHash(fragment.toJsString())
    }

    /** 订阅片段变化；宿主恢复链接或用户后退时触发。 */
    fun subscribe(listener: () -> Unit) = onHashChange(listener)
}
