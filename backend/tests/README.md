# 来源 CRUD 集成验收

`source_crud.rs` 使用真实 PostgreSQL 和全部现有迁移。仅接受本机、名称以 `_test` 结尾的专用数据库；每次创建随机 schema，结束时清除。Keyring 替身通过 Unix socket 返回随机票据，仅验证密文边界和用途隔离，不替代宿主真实加密验收。

```sh
AIO_MEMORY_TEST_DATABASE_URL=postgresql://postgres@127.0.0.1/aio_memory_crud_test \
  cargo test -p az-memory-server --test source_crud --locked -- --ignored
```

覆盖录入、原文读取、修订版本、秘密隔离与 ID 保留、权限拒绝、租约撤销、搜索分页及删除。数据库连接信息通过环境变量提供，不提交真实凭据。
