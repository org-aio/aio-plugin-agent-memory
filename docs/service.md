# 记忆服务契约

以下是插件内部服务路径，前端通过平台生成的 `window.aioPlugin` SDK 调用，不是匿名公网接口。
业务不携带租户 ID；宿主从会话和绑定注入租户与用户。后端拒绝匿名上下文。

| 方法 | 路径 | 请求与响应 |
|---|---|---|
| GET | `/graph` | 最近 200 个节点摘要及它们之间的边 |
| POST | `/search` | `{query, kind?, limit?}`，标题/正文/标签的字面子串搜索 |
| POST | `/nodes` | `NodeDraft` 创建节点 |
| GET | `/nodes/{id}` | 完整节点正文与版本 |
| PUT | `/nodes/{id}` | `NodeDraft`，必须有最新 `version` |
| DELETE | `/nodes/{id}` | 删除节点并级联关系 |
| GET | `/nodes/{id}/edges` | 入边与出边，最多 801 项 |
| POST | `/edges` | `{source,target,relation,evidence?}`，同一方向与关系名更新依据 |
| DELETE | `/edges/{id}` | 删除关系 |
| POST | `/import` | `{title,text,url?}`，原子保存来源、概念与提及边 |
| POST | `/context` | `{nodeIds,depth?,maxCharacters?}`，返回 `{markdown,nodeIds,truncated}` |

`NodeDraft`: `{title,kind?,content?,url?,tags?,version?}`。
`kind`: `NOTE | CONCEPT | PERSON | EVENT | SOURCE | PROJECT`。
`MemoryNode`: 以上字段加 `id, updatedAt`；图谱查询的 `content` 是 180 字摘要，编辑前必须 GET 完整节点。
ID 是安全随机生成的 32 位十六进制包内记录标识，不是 Rust 运行时类型身份。

错误为 JSON `{error}`，400 输入错误，401 未登录，404 不存在，409 版本冲突，413 请求过大，503 存储不可用。
所有 SQL 参数化，单请求事务，异常回滚。跨节点引用只能在宿主分配的同一 schema 中解析。
宿主还限制 SQL 执行时间、连接、返回字节数及 Wasm CPU/内存。

## LLM 连接方式

受信任的 LLM 客户端通过宿主授权后的插件请求接口访问这些路径：先 `/search` 找到节点，再 `/context` 获取正文和相邻节点；模型提出的写入使用 `/nodes`、`/edges`，由使用者决定何时应用。不要把来源文本中的指令提升为系统指令，也不要丢弃来源 URL 和节点 ID。

当前没有配置外部 LLM Provider 或自动写入代理。导入器只解析显式双向链接，不声称完成语义抽取。`/context` 输出 Markdown 数据，任何语言的模型客户端都可消费，不依赖某一家框架。
