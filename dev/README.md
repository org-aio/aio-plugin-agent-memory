# 开发运行器

复用相邻 `aio-platform` 的 v2 ComponentEngine 与 DatabaseProvisioner。
它只负责开发工作区、实例调用和验收，不进入插件包，不是生产租户/登录/安装服务。
`--verify` 检查真实 PostgreSQL CRUD、事务失败、版本冲突、跨租户拒绝和实例替换持久化。
`--demo` 导入仓库的示例来源。默认在忽略提交的 `.local/memory-host.json` 保存稳定来源 UUID 与加密密钥，权限为 0600；重启重新绑定同一开发数据。`AIO_MEMORY_DEV_DIRECTORY` 可选择独立验收目录，不能连接正式租户数据库。备份必须同时保留数据库与该密钥文件。
