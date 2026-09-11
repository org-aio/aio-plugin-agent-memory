# 开发运行器

复用相邻 `aio-platform` 的 v2 ComponentEngine 与 DatabaseProvisioner。
它只负责开发工作区、实例调用和验收，不进入插件包，不是生产租户/登录/安装服务。
`--verify` 检查真实 PostgreSQL CRUD、事务失败、版本冲突、跨租户拒绝和实例替换持久化。
`--demo` 导入仓库的示例来源；每次启动生成独立工作区，不写正式租户数据库。
