# 智能体记忆 / AIO Agent Memory

仓库：`aio-plugin-agent-memory`，父插件：`aio-plugin-agent`。子插件统一使用 `aio-plugin-<父功能>-<子功能>`，本功能名为 `memory`。仓库、发布来源和页面标识不是 Rust 运行时类型身份。

Repository: `aio-plugin-agent-memory`; parent plugin: `aio-plugin-agent`. Sub-plugins use the `aio-plugin-<parent-feature>-<sub-feature>` naming; this feature is `memory`, with crate module `site.addzero.aio.agent.memory`. The repository, release source and page identity are not Rust runtime type identity.

Agent 对话通过受信桥调用本插件。来源、空间权限、秘密隔离和持久整理队列属于 Memory，模型请求由 Agent 的 Pi 常驻服务执行；正式 AIO 宿主管理父子安装、持久激活和跨插件授权，界面沿用 Dioxus。

Agent conversations call this plugin through the trusted bridge. Sources, space permissions, secret isolation and the persistent organization queue belong to Memory; model requests are executed by the Agent's Pi resident service. A production AIO host manages parent/child installation, persistent activation and cross-plugin authorization; the UI uses Dioxus.

独立的全栈记忆插件：Dioxus 图谱界面 + Rust process 后端 + PostgreSQL。
前后端、模型、迁移以一个包发布和回滚，没有 JVM，也没有宿主预设控件协议。

A standalone full-stack memory plugin: a Dioxus graph UI + Rust process backend + PostgreSQL. Frontend, backend, models and migrations publish and roll back as one package — no JVM, and no host-preset widget protocol.

## 功能 / Features

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

- Notes, concepts, people, events, sources and projects are unified as nodes; bodies keep Markdown, and tags are stored as JSONB.
- Directed relations, relation evidence, bidirectional links and a one-hop neighborhood; deleting a node cascades its relations, with confirmation before the operation.
- Import Markdown/text files or paste bodies; explicit `[[标题]]` generates concept nodes and “提及” (mentions) edges.
- Search over titles, bodies and tags; local graph filtering, drag, pan, zoom, pause and list switching.
- Context retrieval expands along 0 to 3 hops of relations and returns bodies, source URLs, node IDs and relations, ready for LLM clients.
- Edits reject stale overwrites with version numbers; imports and relation writes happen in transactions.
- Conversations, quick notes and imports share the same secret-isolation rules, keeping three layers — encrypted originals, sanitized sources and wiki — with passwords and tokens replaced by unguessable references in separate encrypted fields. Material suspected of containing secrets but not reliably splittable is held in secure quarantine.
- Personal/team spaces, manager/editor/reader roles, submitter original-text permissions and separate secret authorization. Worker processes cannot call original-text or secret-display interfaces.
- Persistent leases, backoff retries, preemption pauses, model binding, wiki auto-revision, aliases, source-version evidence, pending conflicts and history rollback. Model submission and task state commit in one transaction; expired or duplicate leases never regenerate knowledge.
- The workbench provides Wiki, Graph, Sources, Credentials and Pending-Organization views; the original conversation can add explicit notes such as “上一条是密码” (the previous message is a password) to keep treating an entire quarantined passage as a secret.
- The shared layer provides a model-free conservative conversation classifier: explicit lookups directly recall sanitized excerpts, saves only queue background organization, and complex or ambiguous input goes to the Agent model. Lookup sources are marked `recorded`, keeping encrypted originals and conversation evidence without entering normal graph, retrieval or wiki-organization queues.
- `/route` returns this turn's direct hits and context nodes; `/activation` provides a controlled graph that preferentially includes the activation neighborhood, for Agent chat integration.

这里不内置 LLM 服务商或密钥，模型配置由 Agent 提供。检索覆盖标题、别名、正文和图谱邻域，包括尚未完成 wiki 整理的净化资料；没有向量服务依赖。用户内容始终作为资料，不能改变宿主或模型系统规则。任意未标注密码无法保证被自动识别，凭据自动调用外部服务不在此版范围。

No LLM provider or key is bundled here; model configuration is provided by the Agent. Retrieval covers titles, aliases, bodies and graph neighborhoods, including sanitized material not yet wiki-organized; there is no vector-service dependency. User content is always treated as data and cannot change host or model system rules. Arbitrary unmarked passwords are not guaranteed to be auto-recognized, and automatically calling external services with credentials is out of scope for this version.

## 结构 / Structure

```text
frontend/    Dioxus Web 工作台、图谱、编辑 Dialog
backend/     Rust process、数据库事务、检索、SQL 迁移
shared/      无 UI 依赖的模型与验证规则
dev/         仅供开发和验收的 AIO v2 运行器，不进入插件包
scripts/     作者侧构建、预览、浏览器测试
examples/    可选的演示来源文档
```

```text
frontend/    Dioxus Web workbench, graph, edit dialogs
backend/     Rust process, database transactions, retrieval, SQL migrations
shared/      UI-free models and validation rules
scripts/     author-side build and packaging
examples/    optional demo source documents
```

## 构建 / Build

构建插件需要 Rust、Dioxus CLI 与相邻的 AIO v2 平台工作区。前端使用 Dioxus 0.7.9 与共享 `az-ui-components`，后端使用 Axum、SQLx 和平台 process 契约。

Building the plugin requires Rust, the Dioxus CLI and the adjacent AIO v2 platform workspace. The frontend uses Dioxus 0.7.9 and shared `az-ui-components`; the backend uses Axum, SQLx and the platform process contract.

```sh
cargo check --locked --offline
dx build --package az-memory-frontend --platform web --release --locked --offline
./scripts/build.sh --process
```

旧 Kotlin/Wasm Component 构建已移除。数据库迁移、业务表和密文用途字符串保持兼容；密文读写通过宿主 process Broker 的 Keyring 端点完成，历史 `aio:plugin/cryptography` 信封仍可读取。

The legacy Kotlin/Wasm Component build has been removed. Database migrations, business tables and ciphertext purpose strings remain compatible; encryption reads and writes go through the host process broker's Keyring endpoints, so historical `aio:plugin/cryptography` envelopes remain readable.

## 验收 / Acceptance

构建后由支持 `aio:plugin@2.0.0` 的开发宿主或正式宿主加载 `dist/frontend` 与 `dist/memory-server`，再在隔离租户安装整包。验收必须检查 Dioxus 页面挂载、数据库迁移、跨插件调用、Keyring 读写、来源导入和租户授权；只看构建成功不能代替运行时验收。

After build, load `dist/frontend` and `dist/memory-server` in a development or production host supporting `aio:plugin@2.0.0`, then install the whole package in an isolated tenant. Acceptance must cover Dioxus page mounting, database migrations, cross-plugin calls, Keyring access, source import and tenant authorization; a successful build is not runtime acceptance.

## 发布边界 / Release Boundaries

`aio-delivery.toml` 声明官方自动构建配方：使用 rust 环境执行 `scripts/build.sh --process`。默认分支推送后，正式宿主按官方发布账号发现、构建并上架，已有安装沿用原数据库绑定。

`aio-delivery.toml` declares the official auto-build recipe: run `scripts/build.sh --process` in the rust environment. After a default-branch push, the production host discovers, builds and publishes through the official release account; existing installs keep their original database bindings.

包清单是 `aio-plugin.toml`，运行时元数据由 process `/aio/describe` 导出，页面入口为 `index.html`。
**仅接受支持 `aio:plugin@2.0.0`、数据库、加密能力、process 执行与 v2 整包安装的宿主。正式 AIO 市场中先启用父插件“智能体”，再安装“智能体记忆”；父插件需要宿主的 v2 process 执行能力。**

The package manifest is `aio-plugin.toml`; runtime metadata is exported by the process `/aio/describe` and the page entry is `index.html`. **Only hosts supporting `aio:plugin@2.0.0`, database, encryption capabilities, process execution and v2 whole-package install are accepted. In the production AIO marketplace, enable the parent plugin “智能体” (Agent) first, then install “智能体记忆” (Agent Memory); the parent plugin needs the host's v2 process execution capability.**

进入“工作空间 → 智能体”即可对话收件和查看本轮激活图谱；独立记忆工作台位于“工作空间 → 记忆图谱”。模型未配置时继续保存加密资料并支持本地检索，wiki 整理等待空间绑定可用模型。生产发布与数据库副本验收记录见 [AIO 宿主部署文档](https://github.com/zjarlin/aio-idea/blob/main/deploy/252/README.md)。

Open “工作空间 → 智能体” (Workspace → Agent) to converse and view this turn's activated graph; the standalone memory workbench is at “工作空间 → 记忆图谱” (Workspace → Memory Graph). When no model is configured, encrypted material still saves and local retrieval works; wiki organization waits for a usable model bound to the space. Production release and database-replica acceptance records are in the [AIO 宿主部署文档](https://github.com/zjarlin/aio-idea/blob/main/deploy/252/README.md) (AIO host deployment doc).

数据库按插件与租户独立 schema/角色隔离；单次图谱最多 200 节点、800 边，上下文最多 24 节点，截断会显式返回。正式数据备份与 schema 兼容回滚由宿主管理，卸载不应默认删除业务数据。

The database is isolated per plugin and tenant by schema/role; a single graph caps at 200 nodes and 800 edges, context at 24 nodes, with truncation returned explicitly. Production data backups and schema-compatible rollback are managed by the host; uninstall should not delete business data by default.

接口与限制见 [服务契约](docs/service.md)。

Interface and limits are documented in [服务契约](docs/service.md) (service contract).
