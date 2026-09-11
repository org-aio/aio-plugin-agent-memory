use super::*;
use az_plugin_runtime::CompiledComponent;

pub async fn verify(
    engine: &ComponentEngine,
    compiled: &CompiledComponent,
    instance: &mut ComponentInstance,
    scope: InvocationScope,
    resources: InvocationResources,
    provisioner: &DatabaseProvisioner,
    migration: &str,
) -> Result<()> {
    let created = call(instance, "POST", "/nodes", json!({"title":"PostgreSQL", "kind":"CONCEPT", "content":"持久化记忆", "tags":["database"]}), "preview").await?;
    ensure!(created["status"] == 201, "创建失败: {created}");
    let node = created["body"].clone();
    let id = node["id"].as_str().unwrap();
    let read = call(
        instance,
        "GET",
        &format!("/nodes/{id}"),
        Value::Null,
        "preview",
    )
    .await?;
    ensure!(read["body"]["content"] == "持久化记忆", "正文未保存");
    let updated = call(
        instance,
        "PUT",
        &format!("/nodes/{id}"),
        json!({"title":"PostgreSQL", "kind":"CONCEPT", "content":"更新正文", "version":1}),
        "preview",
    )
    .await?;
    ensure!(
        updated["body"]["version"] == 2,
        "乐观锁版本未增长: {updated}"
    );
    let stale = call(
        instance,
        "PUT",
        &format!("/nodes/{id}"),
        json!({"title":"stale", "version":1}),
        "preview",
    )
    .await?;
    ensure!(stale["status"] == 409, "未拒绝过期更新");
    let invalid = call(
        instance,
        "POST",
        "/nodes",
        json!({"title":"", "content":"invalid"}),
        "preview",
    )
    .await?;
    ensure!(invalid["status"] == 400, "输入校验失败");
    let imported = call(instance, "POST", "/import", json!({"title":"设计笔记", "text":"[[PostgreSQL]] 保存 [[知识图谱]]，[[PostgreSQL]] 是来源。", "url":"https://example.com/note"}), "preview").await?;
    ensure!(
        imported["status"] == 201 && imported["body"]["linkedNodes"] == 2,
        "导入失败: {imported}"
    );
    let source = imported["body"]["source"]["id"].as_str().unwrap();
    let context = call(
        instance,
        "POST",
        "/context",
        json!({"nodeIds":[source],"depth":1}),
        "preview",
    )
    .await?;
    ensure!(
        context["status"] == 200 && context["body"]["nodeIds"].as_array().unwrap().len() == 3,
        "邻域检索失败: {context}"
    );
    ensure!(
        context["body"]["markdown"]
            .as_str()
            .unwrap()
            .contains("https://example.com/note"),
        "导出缺少引用来源"
    );
    let search = call(
        instance,
        "POST",
        "/search",
        json!({"query":"更新正文"}),
        "preview",
    )
    .await?;
    ensure!(
        search["body"]["nodes"].as_array().unwrap().len() == 1,
        "正文搜索失败: {search}"
    );
    let literal = call(
        instance,
        "POST",
        "/search",
        json!({"query":"%' OR 1=1 --"}),
        "preview",
    )
    .await?;
    ensure!(
        literal["body"]["nodes"].as_array().unwrap().is_empty(),
        "搜索参数未隔离"
    );
    let before = call(instance, "GET", "/graph", Value::Null, "preview").await?;
    let self_link = call(
        instance,
        "POST",
        "/edges",
        json!({"source":id,"target":id,"relation":"自身"}),
        "preview",
    )
    .await?;
    ensure!(self_link["status"] == 400, "未拒绝自关联");
    let missing = call(
        instance,
        "POST",
        "/edges",
        json!({"source":id,"target":"00000000000000000000000000000000","relation":"错误"}),
        "preview",
    )
    .await?;
    ensure!(missing["status"] == 404, "未拒绝不存在的目标");
    let after = call(instance, "GET", "/graph", Value::Null, "preview").await?;
    ensure!(before == after, "失败写入改变了活动图谱");

    let second_database = provisioner
        .create(&scope.source_id, "second", migration)
        .await?;
    let second_scope = InvocationScope {
        context: super::context("second"),
        ..scope.clone()
    };
    ensure!(
        engine
            .instantiate(compiled, second_scope.clone(), resources.clone())
            .await
            .is_err(),
        "绑定可跨租户复用"
    );
    let mut second = engine
        .instantiate(
            compiled,
            second_scope,
            InvocationResources {
                database: Some(second_database),
                ..Default::default()
            },
        )
        .await?;
    let isolated = call(
        &mut second,
        "GET",
        &format!("/nodes/{id}"),
        Value::Null,
        "second",
    )
    .await?;
    ensure!(isolated["status"] == 404, "跨租户读取成功");
    ensure!(
        call(instance, "GET", "/graph", Value::Null, "second")
            .await
            .is_err(),
        "伪造请求上下文未被拒绝"
    );
    let mut denied = engine
        .instantiate(
            compiled,
            InvocationScope {
                grants: Default::default(),
                ..scope.clone()
            },
            resources.clone(),
        )
        .await?;
    let denied_read = call(&mut denied, "GET", "/graph", Value::Null, "preview").await?;
    ensure!(denied_read["status"] == 503, "未授权数据库调用成功");

    // 销毁原实例后以同一数据库绑定创建新版本，业务数据仍由 PostgreSQL 提供。
    instance.lifecycle(Phase::Deactivate).await?;
    let mut replacement = engine.instantiate(compiled, scope, resources).await?;
    replacement.health().await?;
    ensure!(
        call(&mut replacement, "GET", "/graph", Value::Null, "preview").await? == before,
        "替换实例后数据丢失"
    );
    let deleted = call(
        &mut replacement,
        "DELETE",
        &format!("/nodes/{source}"),
        Value::Null,
        "preview",
    )
    .await?;
    ensure!(deleted["status"] == 204, "删除失败");
    let clean = call(&mut replacement, "GET", "/graph", Value::Null, "preview").await?;
    ensure!(
        clean["body"]["edges"].as_array().unwrap().is_empty(),
        "节点删除后关系未级联移除"
    );
    println!(
        "Memory integration: CRUD, import, links, search, context, validation, optimistic lock, tenant isolation, denied capability, replacement persistence, cascade delete passed"
    );
    Ok(())
}
