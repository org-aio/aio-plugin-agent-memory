# 验证记录

本地验证日期：2026-09-23。Rust 1.95.0，Dioxus 0.7.9，Dioxus CLI 0.7.9。

- `cargo test --workspace --locked`：共享模型、Rust process 后端 4 项测试通过。
- `cargo fmt --all -- --check` 与 release 构建通过。
- `scripts/build.sh --process` 生成 Dioxus Web 静态资源与 `x86_64-unknown-linux-gnu.2.17` 服务端二进制。
- GitHub Actions `Build Agent Memory` 对迁移提交执行 workspace 测试并通过。
- 旧 Kotlin/Wasm Component 源码与构建链已移除；数据库迁移、表名及历史密文用途保持兼容。

## 验收边界

- 正式发布、跨运行时整包升级和公网页面必须在真实 AIO 宿主中验证安装 revision、process 健康、Dioxus 页面挂载及业务请求。
- 本地构建通过不代表市场已发布、租户已升级或页面已可访问。
