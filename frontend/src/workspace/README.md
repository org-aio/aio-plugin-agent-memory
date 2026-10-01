# 记忆工作台

`MemoryWorkspace` 提供空间切换与记忆视图，`notes` 负责随心记 CRUD，`ssh.rs` 提供 SSH 连接视图：列出主机、新建/编辑、写入配对设备、验证免密并展示本机公钥。私钥和密码不经过前端。布局样式在 `style.css`，复用组件库语义色和控件；`.memory-app` 占满 iframe 高度并独立纵向滚动，避免公共样式的 `body { overflow: hidden }` 截断内容。列表上方的分页栏吸顶，详情弹窗正文独立滚动。
