package site.addzero.aio.agent.memory.storage

import site.addzero.aio.agent.memory.bindings.Database
import site.addzero.aio.agent.memory.bindings.Host

internal class DatabaseSession(val transaction: UInt) {
    fun query(sql: String, values: List<Database.Value> = emptyList()) =
        Database.query(transaction, sql, values).getOrThrow().values

    fun execute(sql: String, values: List<Database.Value> = emptyList()) =
        Database.execute(transaction, sql, values).getOrThrow()

    fun id() = Host.random(16U).getOrThrow().joinToString("") { it.toString(16).padStart(2, '0') }

    fun now() = Host.now().toLong()

    companion object {
        fun <T> transaction(block: (DatabaseSession) -> T): T {
            val id = Database.begin().getOrThrow()
            return try {
                val value = block(DatabaseSession(id))
                Database.finish(id, true).getOrThrow()
                value
            } catch (failure: Throwable) {
                Database.finish(id, false)
                throw failure
            }
        }
    }
}

internal fun text(value: String) = Database.Value.Text(value)

internal fun number(value: Long) = Database.Value.Integer(value)

internal fun bool(value: Boolean) = Database.Value.Boolean(value)

internal fun List<Database.Value>.string(index: Int): String =
    when (val value = this[index]) {
        is Database.Value.Text -> value.value
        is Database.Value.Json -> value.value
        else -> error("数据库字段类型不匹配")
    }

internal fun List<Database.Value>.nullableString(index: Int): String? =
    if (this[index] is Database.Value.Null) null else string(index)

internal fun List<Database.Value>.long(index: Int) = (this[index] as Database.Value.Integer).value

internal fun List<Database.Value>.boolean(index: Int) =
    (this[index] as Database.Value.Boolean).value

internal fun List<Database.Value>.bytes(index: Int) = (this[index] as Database.Value.Bytes).value
