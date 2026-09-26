# 记忆服务契约

服务通过受信宿主桥调用，租户、用户和工作身份从宿主上下文注入，不能由浏览器请求指定。ID 为安全随机生成的 32 位十六进制记录标识。`spaceId` 查询参数选择空间，省略时使用个人空间；节点、来源和秘密 ID 会重新校验实际归属。

## 收件与整理

| 方法 | 路径 | 契约 |
|---|---|---|
| POST | `/capture` | `{requestId,text,spaceId?,origin?,reference?,clarifies?}` 返回 202 `SourceView` |
| GET | `/sources` | `spaceId?,query?,status?,offset?,limit?`，返回 `{sources,total,truncated}`；默认 200 项，limit 为 1..200 |
| GET | `/sources/{id}` | 净化正文、状态、秘密引用与可操作权限 |
| PUT | `/sources/{id}` | `{text,version}`，仍有写权限的交互式提交者修订原文，重新隔离、加密并排队整理 |
| DELETE | `/sources/{id}` | 交互式编辑者删除来源，清除原文、凭据和旧任务租约，停止相关知识召回 |
| POST | `/sources/{id}/original` | 仅交互式提交者，返回 `{value}` |
| POST | `/sources/{id}/retry` | 失败、待整理或冲突任务重新排队 |
| POST | `/tasks/claim` | `{spaceId}`，工作进程领取净化来源、候选条目、模型绑定和租约 |
| POST | `/tasks/{id}/submit` | `{lease,result}`，校验后原子写入知识及状态 |
| POST | `/tasks/{id}/fail` | `{lease,code}`，支持 `cancelled`、`invalid_result`、模型不可用 |
| GET | `/sources/{id}/proposal` | 编辑者读取待核实模型修订 |
| POST | `/sources/{id}/resolve` | `{accept,versions?}`，接受修订必须携带查看过的条目版本 |
| POST | `/import` | `{requestId,title,text,url?}`，文件/文本采用同一隔离管线，重试不重复创建来源 |

`SourceView` 只含净化内容，状态为 pending、processing、complete、quarantined、conflict、failed 或 recorded。recorded 表示明确查找的对话来源，保留收件但不生成 wiki 任务，也不参与普通检索与图谱。秘密字段用 `[[secret:字段ID]]` 替代；字段 ID 的归属由服务端校验。原文以宿主版本化密钥加密，不进入检索、图谱或模型任务。

来源列表只搜索净化标题和正文，`query` 最多 256 字符，通配符按字面处理；`offset` 从 0 开始，按内容更新时间和 ID 稳定排序。返回的 `title`、`version`、`origin`、`canEdit`、`canDelete` 用于笔记展示和操作，服务端仍重新鉴权。修改必须带当前版本，冲突返回 409；未变化的秘密字段保留 ID 和独立授权，移除的字段及其授权一并删除。编辑与删除先锁整理任务，再修改来源，旧租约提交不能恢复旧内容。

`origin` 为 chat/note/import。`clarifies` 指向同一原对话、同一提交者的保密暂存资料；仅明确的整段秘密用途说明可以解除暂存，未识别说明不改变原来源。加密原文保留，净化版本和说明来源留痕。

工作调用需要受信服务身份及 `memory:compile` 授权。租约 180 秒，失败最多 5 次并退避，暂停不计失败。未绑定模型不会领取整理任务，仍可收件与检索净化来源。执行和提交均重新检查成员及原提交者写权限。

## 空间与秘密

| 方法 | 路径 | 契约 |
|---|---|---|
| GET/POST | `/spaces` | 列表或 `{title,modelBinding?}` 新建团队空间 |
| PUT | `/spaces/{id}` | 管理者修改名称和模型绑定 |
| GET/POST | `/spaces/{id}/members` | 管理者查询或 `{userId,role}` 修改成员 |
| DELETE | `/spaces/{id}/members/{user}` | 移除成员，同时撤销该空间秘密授权 |
| GET | `/secrets` | 字段引用、标签、来源及 canReveal/canManage，不含明文 |
| POST | `/secrets/{id}/reveal` | 交互式独立授权后返回 `{value}` |
| PUT | `/secrets/{id}/grants` | 秘密管理者提交 `{userId,reveal,manage}` |

角色为 OWNER/EDITOR/READER。普通空间管理员不自动获得原文或秘密权限。原文默认只允许提交者读取，秘密所有者或被单独授权者可查看；后台工作身份不能读取明文。将授权布尔值设为 false 可撤销相应权限。

## 知识与检索

| 方法 | 路径 | 契约 |
|---|---|---|
| GET | `/graph` | 最近 200 个节点摘要和至多 800 条关系 |
| POST | `/search` | `{query,kind?,limit?}`，标题、正文、标签、别名搜索 |
| POST | `/recall` | `{query,limit?,excludeIds?}`，关键词与图谱检索候选，至多 24 项 |
| POST | `/route` | `{sourceId}`，仅从当前空间已净化来源分类，返回 route、reply、context、citations、matchedNodeIds、activatedNodeIds |
| POST | `/activation` | `{nodeIds?}`，至多 24 个种子；优先包含种子和一层邻域，返回至多 120 节点、800 边，正文为空，过期或不可见种子被移除 |
| POST | `/visibility` | `{nodeIds}`，至多 400 个 ID，返回当前仍可见的 ID |
| POST/PUT | `/nodes`、`/nodes/{id}` | `NodeDraft`，编辑必须带当前 version |
| GET/DELETE | `/nodes/{id}` | 完整节点或删除；来源删除清除密文并停止派生内容召回 |
| GET | `/nodes/{id}/edges`、`/nodes/{id}/sources` | 关系或来源依据 |
| GET | `/nodes/{id}/revisions` | 版本、作者、来源和净化修订 |
| POST | `/nodes/{id}/rollback` | `{version,currentVersion}`，创建新的人工修订 |
| POST/DELETE | `/edges`、`/edges/{id}` | 关系 `{source,target,relation,evidence?}` 或删除 |
| POST | `/context` | `{nodeIds,depth?,maxCharacters?}` 返回 `{markdown,nodeIds,truncated}` |

`NodeDraft` 为 `{title,kind?,content?,url?,tags?,aliases?,version?}`。SOURCE 只能通过收件管线写入，普通编辑发现疑似秘密会拒绝。模型结果只允许 NOTE/CONCEPT/PERSON/EVENT/PROJECT，最多 24 条目、48 关系；人工编辑、版本冲突及模型标记的不确定事实进入待核实。

所有请求采用数据库事务并重新鉴权。错误为 `{error}`：400 输入错误、401 未登录、403 无权访问、404 不存在、409 版本/租约冲突、413 过大、503 暂不可用。普通响应不携带原文，秘密展示响应禁止缓存。自动识别不能保证发现任意未标注密码。
