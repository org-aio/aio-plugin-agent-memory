# 智能体记忆

仓库：`aio-plugin-agent-memory`，父插件：`aio-plugin-agent`。子插件统一使用 `aio-plugin-<父功能>-<子功能>`，本功能名为 `memory`；Kotlin 命名空间为 `site.addzero.aio.agent.memory`。仓库、发布来源和页面标识不是 Rust 运行时类型身份。

Agent 对话通过受信桥调用本插件。来源、空间权限、秘密隔离和持久整理队列属于 Memory，模型请求由 Agent 的 Pi 常驻服务执行；正式 AIO 宿主管理父子安装、持久激活和跨插件授权，界面沿用 Compose。

独立的全栈记忆插件：真实 Compose 图谱界面 + Kotlin Wasm Component 后端 + PostgreSQL。
前后端、模型、迁移以一个包发布和回滚，没有 JVM，也没有宿主预设控件协议。

## 功能

- 笔记、概念、人物、事件、来源、项目统一为节点，正文保留 Markdown，标签用 JSONB 保存。
- 有方向的关系、关系依据、双向关联与一层邻域；删除节点级联删除关系，操作前确认。
- 导入 Markdown / 文本文件或粘贴正文，显式 `[[标题]]` 生成概念节点与「提及」边。
- 标题、正文和标签搜索；本地图谱筛选、拖动、平移、缩放、暂停与列表切换。
- 上下文检索沿 0 至 3 层关系展开，返回正文、来源 URL、节点 ID 与关系，可供 LLM 客户端使用。
- 编辑使用版本号拒绝过期覆盖；导入和关系写入在事务中完成。
- 对话、速记与导入共用秘密隔离规则，保留加密原文、净化来源、wiki 三层数据；密码和 Token 替换成独立加密字段的不可猜测引用。疑似但无法可靠拆分的资料保密暂存。
- 个人/团队空间、管理者/编辑者/阅读者、提交者原文权限及秘密单独授权。工作进程不能调用原文或秘密展示接口。
- 持久租约、退避重试、抢占暂停、模型绑定、wiki 自动修订、别名、来源版本依据、冲突待核实和历史回退。模型提交和任务状态在同一事务提交，过期或重复租约不会重复生成知识。
- 工作台提供 Wiki、图谱、来源、凭据和待整理视图；原对话可补充“上一条是密码”等明确说明，把保密暂存的整段资料作为秘密继续处理。
- 共享层提供无模型的保守对话分类器，明确查找直接召回净化摘录，保存仅排后台整理，复杂或不明确输入交给 Agent 模型。查找来源标为 recorded，保留加密原文和对话依据，不进入普通图谱、检索或 wiki 整理队列。
- `/route` 返回本轮直接命中与上下文节点，`/activation` 提供优先包含激活邻域的受控图谱，供 Agent 聊天联动使用。

这里不内置 LLM 服务商或密钥，模型配置由 Agent 提供。检索覆盖标题、别名、正文和图谱邻域，包括尚未完成 wiki 整理的净化资料；没有向量服务依赖。用户内容始终作为资料，不能改变宿主或模型系统规则。任意未标注密码无法保证被自动识别，凭据自动调用外部服务不在此版范围。

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
cargo build --locked --release --manifest-path dev/Cargo.toml
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
dev/target/release/aio-agent-memory-dev --verify
npm run preview -- --demo
npm run test:browser
```

默认地址 `http://127.0.0.1:4191/`，`PORT` 可覆盖。开发运行器在忽略提交的 `.local/memory-host.json` 保存稳定来源 UUID 和宿主密钥，以平台持久绑定恢复同一数据库角色和 schema；`AIO_MEMORY_DEV_DIRECTORY` 可选择另一个目录。只有 `--verify` 使用独立临时来源，`--demo` 才导入演示资料。必须一并保留数据库和密钥文件。

预览只监听 loopback，以随机挂载票据校验请求，用平台原版 v2 通信桥和沙箱 iframe。数据库凭据只存在于开发宿主进程，前端和 Wasm 都拿不到。图谱选择先更新本地状态，随后读取该节点完整正文；拖动、缩放、布局和视图切换不发请求。

## 发布边界

`aio-delivery.toml` 声明官方自动构建配方：使用 fullstack 环境执行 `scripts/build.sh`。默认分支推送后，正式宿主按官方发布账号发现、构建并上架，已有安装沿用原数据库绑定。

包清单是 `aio-plugin.toml`，运行时元数据由 Component `describe` 导出，页面入口为 `index.html`。
**仅接受支持 `aio:plugin@2.0.0`、数据库、加密能力与 v2 整包安装的宿主。正式 AIO 市场中先启用父插件“智能体”，再安装“智能体记忆”；父插件需要宿主的 v2 process 执行能力。**

进入“工作空间 → 智能体”即可对话收件和查看本轮激活图谱；独立记忆工作台位于“社区插件 → 智能体 → 记忆图谱”。模型未配置时继续保存加密资料并支持本地检索，wiki 整理等待空间绑定可用模型。生产发布与数据库副本验收记录见 [AIO 宿主部署文档](https://github.com/zjarlin/aio-idea/blob/main/deploy/252/README.md)。
本仓库 `dev/` 依赖相邻平台工作区的 v2 crates，平台至少需要包含提交 `01f8fc4`（持久绑定、加密、激活和受控约束迁移）；旧版平台不能运行该验收工具。

前端包含本地 Noto Sans CJK 字体及 OFL 许可证，加载不需要公网字体/CDN。数据库按插件与租户独立 schema/角色隔离；单次图谱最多 200 节点、800 边，上下文最多 24 节点，截断会显式返回。正式数据备份与 schema 兼容回滚由宿主管理，卸载不应默认删除业务数据。

接口与限制见 [服务契约](docs/service.md)。
