# AIO Agent Memory

仓库：`aio-plugin-agent-memory`，父插件：`aio-plugin-agent`。子插件统一使用 `aio-plugin-<父功能>-<子功能>`，本功能名为 `memory`；Kotlin 命名空间为 `site.addzero.aio.agent.memory`。仓库、发布来源和页面标识不是 Rust 运行时类型身份。

本次先规范仓库归属与命名，记忆服务仍保留原有前后端实现和正式数据。父插件的自动组合安装、跨插件检索授权与联合回滚尚未接入，不以命名文件冒充宿主生命周期支持。

独立的全栈记忆插件：真实 Compose 图谱界面 + Kotlin Wasm Component 后端 + PostgreSQL。
前后端、模型、迁移以一个包发布和回滚，没有 JVM，也没有宿主预设控件协议。

## 功能

- 笔记、概念、人物、事件、来源、项目统一为节点，正文保留 Markdown，标签用 JSONB 保存。
- 有方向的关系、关系依据、双向关联与一层邻域；删除节点级联删除关系，操作前确认。
- 导入 Markdown / 文本文件或粘贴正文，显式 `[[标题]]` 生成概念节点与「提及」边。
- 标题、正文和标签搜索；本地图谱筛选、拖动、平移、缩放、暂停与列表切换。
- 上下文检索沿 0 至 3 层关系展开，返回正文、来源 URL、节点 ID 与关系，可供 LLM 客户端使用。
- 编辑使用版本号拒绝过期覆盖；导入和关系写入在事务中完成。

这里没有内置 LLM 服务商、密钥或虚构的模型调用。当前提供结构化记忆 CRUD、显式链接提取和带引用的上下文 API；模型自动抽取、摘要、对话和向量检索尚未实现。用户内容始终是检索资料，不是宿主或模型系统指令。

## 结构

```text
frontend/    Compose wasmJs 界面、图谱适配、编辑 Dialog
backend/     Kotlin Component、数据库事务、检索、SQL 迁移
shared/      无 UI 依赖的模型与验证规则
graph/       指定 az-compose 图谱组件的锁定源码依赖
dev/         仅供开发和验收的 AIO v2 运行器，不进入插件包
scripts/     作者侧构建、预览、浏览器测试
examples/    可选的演示来源文档
```

## 构建

构建插件需要 Node.js 22+ 与 `wasm-tools`，开发运行器另需 Rust 和相邻的 AIO v2 平台工作区。Kotlin wrapper 固定为 0.12.0-dev-4233 并校验分发摘要，编译器为 2.4.10，Compose 为 1.12.0-beta03。

```sh
npm ci --ignore-scripts
npm run build
./kotlin test -m shared -p jvm
./kotlin test -m graph -p jvm
cargo build --locked --manifest-path dev/Cargo.toml
```

`graph/source.lock.json` 固定 `az-compose` 的 Git SHA。构建时读取该提交的通用图谱文件，不维护组件源码副本；可设置 `AIO_GRAPH_SOURCE` 使用已有本地 Git 缓存。`--working-tree` 只用于组件联调，发布必须用锁定提交重新构建。

**图谱上游 `az-compose` 当前为私有仓库，构建需要它的只读访问权限。** 本机可使用 `AIO_GRAPH_SOURCE=../kmp-aio/lib/compose/az-compose npm run build`；该方式仍按锁定提交读取，不会带入未提交改动。公开插件仓库不包含该私有源码。GitHub CI 目前缺少上游只读权限，完整构建尚未通过；不要把个人令牌提交到代码或为解决构建擅自改变上游可见性。

WIT 绑定来自 `aio-platform/lib/plugin/contract/wit/plugin.wit`。`backend/contract/` 保存逐字复制、摘要锁定的 SDK 契约快照，使插件可以独立构建；不是另一个自定义协议。设置 `AIO_PLATFORM` 时还会检查平台契约是否一致。使用 Kotlin 官方 `wit-bindgen` 分支提交 `700f2db5e1d01f7bee8d756750c6f631171f520e` 生成：

```sh
WIT_BINDGEN=/path/to/wit-bindgen sh scripts/generate-bindings.sh
```

## 本地预览与验收

准备一个**独立的开发 PostgreSQL 数据库**。AIO v2 provisioner 需要管理员连接创建最小权限角色，并要求撤销该数据库 public schema 的 PUBLIC 权限。不要对现有业务数据库直接执行这一变更。

```sql
REVOKE ALL ON SCHEMA public FROM PUBLIC;
```

```sh
export AIO_TEST_DATABASE_URL='postgres://developer@127.0.0.1:55432/memory_dev'
dev/target/debug/aio-agent-memory-dev --verify
npm run preview -- --demo
npm run test:browser
```

默认地址 `http://127.0.0.1:4191/`，`PORT` 可覆盖。开发运行器每次启动创建新的隔离工作区，旧工作区数据仍留在 PostgreSQL；浏览器刷新和实例替换不丢数据。`--demo` 才导入演示资料。正式安装应使用宿主持久化的插件/租户绑定，不能拿此开发运行器替代生产安装管理。

预览只监听 loopback，以随机挂载票据校验请求，用平台原版 v2 通信桥和沙箱 iframe。数据库凭据只存在于开发宿主进程，前端和 Wasm 都拿不到。图谱选择先更新本地状态，随后读取该节点完整正文；拖动、缩放、布局和视图切换不发请求。

## 发布边界

包清单是 `aio-plugin.toml`，运行时元数据由 Component `describe` 导出，页面入口为 `index.html`。
**仅接受支持 `aio:plugin@2.0.0`、数据库能力与 v2 整包安装的宿主。当前公网壳尚未完成 v2 数据库与激活迁移，不能上传到旧运行时冒充已部署。**
本仓库 `dev/` 依赖平台工作区的 v2 crates；它们仍在平台重构工作区中，独立克隆旧版平台不能运行该验收工具。

前端包含本地 Noto Sans CJK 字体及 OFL 许可证，加载不需要公网字体/CDN。数据库按插件与租户独立 schema/角色隔离；单次图谱最多 200 节点、800 边，上下文最多 24 节点，截断会显式返回。正式数据备份与 schema 兼容回滚由宿主管理，卸载不应默认删除业务数据。

接口与限制见 [服务契约](docs/service.md)。
