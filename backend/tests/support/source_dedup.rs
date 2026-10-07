use super::*;

pub(super) async fn run(
    service: Arc<MemoryService>,
    pool: sqlx::PgPool,
    owner: MemoryContext,
    space: String,
) -> Result<()> {
    let capture = |text: &str, origin: &str| {
        service::intake::capture(
            State(service.clone()),
            owner.clone(),
            Json(CaptureRequest {
                request_id: uuid::Uuid::new_v4().to_string(),
                text: text.into(),
                space_id: Some(space.clone()),
                origin: origin.into(),
                reference: String::new(),
                clarifies: None,
                images: Vec::new(),
                deduplicate: false,
            }),
        )
    };
    let text = "去重验收\n正文";
    let (first, second) = tokio::join!(capture(text, "note"), capture(text, "note"));
    assert_ne!(first.is_ok(), second.is_ok());
    let (saved, rejected) = if first.is_ok() {
        (first, second)
    } else {
        (second, first)
    };
    assert!(matches!(rejected, Err(MemoryError::Input(message)) if message.contains("内容已存在")));
    let saved = saved.map_err(service_error)?.0;
    assert!(capture(" \t去重验收\r\n正文\n", "import").await.is_err());
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM plugin_memory_sources s JOIN plugin_memory_nodes n ON n.id=s.id WHERE n.content=$1")
        .bind(text).fetch_one(&pool).await?, 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM plugin_memory_tasks WHERE id=$1")
            .bind(&saved.id)
            .fetch_one(&pool)
            .await?,
        1
    );

    // 同一请求原样重试成功，改内容仍被拒绝。
    let request_id: String =
        sqlx::query_scalar("SELECT request_id FROM plugin_memory_sources WHERE id=$1")
            .bind(&saved.id)
            .fetch_one(&pool)
            .await?;
    for (body, valid) in [(text, true), ("other", false)] {
        let result = service::intake::capture(
            State(service.clone()),
            owner.clone(),
            Json(CaptureRequest {
                request_id: request_id.clone(),
                text: body.into(),
                space_id: Some(space.clone()),
                origin: "note".into(),
                reference: String::new(),
                clarifies: None,
                images: Vec::new(),
                deduplicate: false,
            }),
        )
        .await;
        assert_eq!(result.is_ok(), valid);
    }
    let other = capture("去重验收\n不同正文", "note")
        .await
        .map_err(service_error)?
        .0;
    let result = service::source_edit::update(
        State(service.clone()),
        Path(other.id.clone()),
        owner.clone(),
        Json(SourceUpdate {
            text: text.into(),
            version: other.version,
        }),
    )
    .await;
    assert!(matches!(result, Err(MemoryError::Input(_))));
    let unchanged = service::intake::get(
        State(service.clone()),
        Path(other.id.clone()),
        owner.clone(),
    )
    .await
    .map_err(service_error)?
    .0;
    assert_eq!(unchanged.version, other.version);
    assert_eq!(unchanged.text, other.text);
    let _ = capture("去重验收\n正 文", "note")
        .await
        .map_err(service_error)?;

    // 重现旧版两次聊天产生相同文本、不同状态的来源，默认审计查询仍保留全部。
    let legacy = capture(text, "chat").await.map_err(service_error)?.0;
    sqlx::query("UPDATE plugin_memory_sources SET status='recorded' WHERE id=$1")
        .bind(&legacy.id)
        .execute(&pool)
        .await?;
    let mut raw_query = query(&space, "去重验收", 0, 2);
    let raw = service::intake::list(
        State(service.clone()),
        Query(raw_query.clone()),
        owner.clone(),
    )
    .await
    .map_err(service_error)?
    .0;
    assert_eq!(raw.total, 4);
    raw_query.distinct = true;
    let first = service::intake::list(
        State(service.clone()),
        Query(raw_query.clone()),
        owner.clone(),
    )
    .await
    .map_err(service_error)?
    .0;
    assert_eq!(first.total, 3);
    assert_eq!(first.sources.len(), 2);
    assert!(first.truncated);
    raw_query.offset = 2;
    let second = service::intake::list(
        State(service.clone()),
        Query(raw_query.clone()),
        owner.clone(),
    )
    .await
    .map_err(service_error)?
    .0;
    assert_eq!(second.total, 3);
    assert_eq!(second.sources.len(), 1);
    assert!(!second.truncated);
    assert!(first.sources.iter().all(|s| s.id != second.sources[0].id));
    raw_query.offset = 0;
    raw_query.status = "recorded".into();
    assert_eq!(
        service::intake::list(State(service.clone()), Query(raw_query), owner.clone())
            .await
            .map_err(service_error)?
            .0
            .total,
        1
    );

    // 不同秘密及不同保密原文不能因净化占位符相同而误判。
    for (body, valid) in [
        ("去重秘密 password: alpha-canary", true),
        ("去重秘密 password: alpha-canary", false),
        ("去重秘密 password: beta-canary", true),
        ("[[secret:unknown-a]]", true),
        ("[[secret:unknown-b]]", true),
        ("[[secret:unknown-a]]", false),
    ] {
        assert_eq!(capture(body, "note").await.is_ok(), valid, "{body}");
    }
    // 共享空间中不同提交者保留所有权；不同空间相互独立。
    for (user, target_space) in [("editor", Some(space.clone())), ("owner", None)] {
        let context = MemoryContext {
            user_id: user.into(),
            ..owner.clone()
        };
        let target_space = if let Some(id) = target_space {
            id
        } else {
            let alternate = MemoryContext {
                user_id: "alternate".into(),
                ..owner.clone()
            };
            let spaces = service::spaces::list(State(service.clone()), alternate)
                .await
                .map_err(service_error)?
                .0;
            let id = spaces[0].id.clone();
            sqlx::query("INSERT INTO plugin_memory_members(space_id,user_id,role) VALUES($1,'owner','EDITOR')").bind(&id).execute(&pool).await?;
            id
        };
        let _ = service::intake::capture(
            State(service.clone()),
            context,
            Json(CaptureRequest {
                request_id: uuid::Uuid::new_v4().to_string(),
                text: text.into(),
                space_id: Some(target_space),
                origin: "note".into(),
                reference: String::new(),
                clarifies: None,
                images: Vec::new(),
                deduplicate: false,
            }),
        )
        .await
        .map_err(service_error)?;
    }
    let disposable = capture("删除后可以重新记下", "note")
        .await
        .map_err(service_error)?
        .0;
    service::nodes::delete(State(service.clone()), Path(disposable.id), owner.clone())
        .await
        .map_err(service_error)?;
    let _ = capture("删除后可以重新记下", "note")
        .await
        .map_err(service_error)?;

    // 两台设备使用不同随机请求 ID，也只能创建一份来源与整理任务。
    let synced = |text: &str| {
        service::intake::capture(
            State(service.clone()),
            owner.clone(),
            Json(CaptureRequest {
                request_id: uuid::Uuid::new_v4().to_string(),
                text: text.into(),
                space_id: Some(space.clone()),
                origin: "import".into(),
                reference: "apple-note:test".into(),
                clarifies: None,
                images: Vec::new(),
                deduplicate: true,
            }),
        )
    };
    let (macbook, macmini) = tokio::join!(
        synced("设备去重\npassword: sync-alpha-canary"),
        synced(" \t设备去重\r\npassword: sync-alpha-canary\n"),
    );
    let macbook = macbook.map_err(service_error)?.0;
    let macmini = macmini.map_err(service_error)?.0;
    assert_eq!(macbook.id, macmini.id);
    assert!(!macmini.text.contains("sync-alpha-canary"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM plugin_memory_tasks WHERE id=$1")
            .bind(&macbook.id)
            .fetch_one(&pool)
            .await?,
        1
    );
    let different = synced("设备去重\npassword: sync-beta-canary")
        .await
        .map_err(service_error)?
        .0;
    assert_ne!(different.id, macbook.id);
    assert_eq!(
        synced("设备去重\npassword: sync-alpha-canary")
            .await
            .map_err(service_error)?
            .0
            .id,
        macbook.id
    );
    Ok(())
}
