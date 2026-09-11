# Agent Memory 开发约定

- 父插件 aio-plugin-agent；当前子插件 aio-plugin-agent-memory。其他子插件一律 aio-plugin-agent-<功能名>，不另起 aio-plugin-<子功能> 顶级名。
- Kotlin 代码属于 site.addzero.aio.agent.memory，数据库表名和现存业务数据不得因仓库改名而重建或清空。

- 前端、后端和 shared 同仓、同版本发布，前端使用真实 Compose。
- 业务模型不依赖 Compose；界面交互优先本地状态，读写才调用服务。
- PostgreSQL 是持久化源，凭据由宿主管理，所有查询参数化。
- 只使用 aio:plugin@2.0.0 契约，不增加旧页面描述或 action 协议兼容层。
- 图谱源码归 az-compose，改上游后锁定提交，不编辑 graph/src 生成目录。
- 按功能组织且每个功能目录保留 README；注释用中文，人工源码不超过 800 行。
- 收尾验证后分别提交并推送本次修改，不包含其他未完成改动。
