use super::*;

pub(super) async fn run(
    service: Arc<MemoryService>,
    pool: sqlx::PgPool,
    owner: MemoryContext,
    space: &str,
) -> Result<()> {
    for (origin, text, expected) in [
        ("chat", "你好！", "recorded"),
        ("chat", "查找 收件分流测试", "recorded"),
        ("chat", "查找 [[收件分流测试]]", "recorded"),
        ("chat", "保存 收件分流测试", "pending"),
        ("chat", "请分析收件分流测试", "pending"),
        ("note", "查找 随心记分流测试", "pending"),
        ("import", "hello", "pending"),
    ] {
        let request = CaptureRequest {
            request_id: uuid::Uuid::new_v4().to_string(),
            text: text.into(),
            space_id: Some(space.into()),
            origin: origin.into(),
            reference: String::new(),
            clarifies: None,
            images: Vec::new(),
            deduplicate: false,
        };
        let captured =
            service::intake::capture(State(service.clone()), owner.clone(), Json(request.clone()))
                .await
                .map_err(service_error)?
                .0;
        assert_eq!(captured.status, expected, "{origin}: {text}");
        let queued: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM plugin_memory_tasks WHERE id=$1)")
                .bind(&captured.id)
                .fetch_one(&pool)
                .await?;
        assert_eq!(queued, expected == "pending");
        // 同一请求重试不能改变记录状态，也不能产生额外任务。
        let retried =
            service::intake::capture(State(service.clone()), owner.clone(), Json(request))
                .await
                .map_err(service_error)?
                .0;
        assert_eq!(retried.id, captured.id);
        assert_eq!(retried.status, expected);
        if expected == "recorded" {
            let edges: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM plugin_memory_edges WHERE source=$1 OR target=$1",
            )
            .bind(&captured.id)
            .fetch_one(&pool)
            .await?;
            assert_eq!(edges, 0);
        }
    }
    Ok(())
}
