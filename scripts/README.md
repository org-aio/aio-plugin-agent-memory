# 开发脚本

仅作者侧构建、校验和测试。安装阶段不执行仓库脚本。

构建脚本执行 Dioxus Web 打包与 Rust process 后端编译。运行验收由具备 process 能力的隔离宿主完成，仓库脚本不执行安装或发布会话；发布凭据不进入构建容器。

前端交互回归在 `dist/frontend` 生成后运行 `node scripts/test-frontend.mjs`。脚本用本地 HTTP 夹具模拟宿主 `<base>` 与 `aioPlugin` 桥，验证“随心记”首屏、`POST /import` 和保存后的来源刷新。
