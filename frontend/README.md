# Dioxus 前端

Dioxus Web 工作台，菜单名称为“记忆”。默认进入“随心记”，支持录入、Markdown 预览、全文详情、原文编辑、确认删除、搜索、状态筛选和分页。笔记卡片只展示净化内容；草稿按空间保留在内存中，失败重试复用请求 ID。Wiki、图谱、来源、凭据和待整理保留为独立视图。所有请求都在沙箱内通过平台 SDK 调用业务服务。

## 本机验收

在仓库根目录执行：

```sh
dx build --package az-memory-frontend --platform web --release --locked
node scripts/package-frontend.mjs
npm install --no-save --package-lock=false playwright
npx playwright install chromium
node scripts/test-frontend.mjs
node scripts/frontend-fixture.mjs
```

预览服务仅监听本机随机空闲端口，启动时输出 URL，使用合成数据和 SDK 替身。测试加载实际 WASM，并复现宿主的沙箱 iframe、资源前缀与 CSP；覆盖 CRUD、Markdown、失败保留草稿、版本冲突、按空间隔离草稿、只读权限、分页搜索及桌面/手机布局。截图输出到忽略目录 `test-results/`。已有 Chromium 时可通过 `PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH` 指定，不在代码中保存本机路径。此验收不代表生产账号或宿主真实接口联调。
