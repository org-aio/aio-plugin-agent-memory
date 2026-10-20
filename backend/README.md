# 记忆后端

Rust process 导出 v2 请求、描述、健康检查和生命周期。存储通过宿主 process 数据库与 Keyring Broker 访问，不包含数据库密码或 JVM。

`/s/{token}` 是清单声明的匿名只读页；它只在宿主公开入口下可用，令牌换取单条已净化来源。`AIO_MEMORY_PUBLIC_BASE` 仅为本地开发覆盖公开前缀。
