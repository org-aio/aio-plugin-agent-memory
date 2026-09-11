mod verification;

use anyhow::{Context, Result, ensure};
use az_plugin_contract::{CapabilityGrants, InvocationScope, RequestContext};
use az_plugin_runtime::{
    ComponentEngine, ComponentInstance, DatabaseProvisioner, InvocationResources,
    bindings::aio::plugin::transport::{Phase, Request},
};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, BufReader};

fn context(tenant: &str) -> RequestContext {
    RequestContext {
        tenant_id: Some(tenant.into()),
        user_id: Some("developer".into()),
        session_id: None,
        request_id: uuid::Uuid::new_v4().to_string(),
    }
}

async fn call(
    instance: &mut ComponentInstance,
    method: &str,
    path: &str,
    body: Value,
    tenant: &str,
) -> Result<Value> {
    let response = instance
        .handle(
            Request {
                method: method.into(),
                path: path.into(),
                query: None,
                headers: vec![],
                body: if body.is_null() {
                    vec![]
                } else {
                    serde_json::to_vec(&body)?
                },
            },
            context(tenant),
        )
        .await?;
    Ok(
        json!({"status": response.status, "body": if response.body.is_empty() { Value::Null } else { serde_json::from_slice::<Value>(&response.body)? }}),
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("缺少插件目录")?;
    let source = format!("memory-dev-{}", uuid::Uuid::new_v4());
    let provisioner =
        DatabaseProvisioner::connect(&std::env::var("AIO_TEST_DATABASE_URL")?).await?;
    let migration = std::fs::read_to_string(root.join("backend/migrations/0001_memory.sql"))?;
    let database = provisioner.create(&source, "preview", &migration).await?;
    let scope = InvocationScope {
        source_id: source.clone(),
        revision: "development".into(),
        context: context("preview"),
        grants: CapabilityGrants {
            database: true,
            ..Default::default()
        },
    };
    let resources = InvocationResources {
        database: Some(database),
        ..Default::default()
    };
    let engine = ComponentEngine::new()?;
    let compiled = engine
        .compile(&std::fs::read(root.join("dist/plugin.wasm"))?)
        .await?;
    let mut instance = engine
        .instantiate(&compiled, scope.clone(), resources.clone())
        .await?;
    ensure!(
        instance.describe().await?.pages[0].entry == "index.html",
        "页面入口不符合清单"
    );
    instance.lifecycle(Phase::Prepare).await?;
    instance.health().await?;
    instance.lifecycle(Phase::Activate).await?;
    if std::env::args().any(|arg| arg == "--verify") {
        return verification::verify(
            &engine,
            &compiled,
            &mut instance,
            scope,
            resources,
            &provisioner,
            &migration,
        )
        .await;
    }
    if std::env::args().any(|arg| arg == "--demo") {
        let source_text = std::fs::read_to_string(root.join("examples/design-note.md"))?;
        let result = call(
            &mut instance,
            "POST",
            "/import",
            json!({"title":"记忆工作台设计记录", "text":source_text}),
            "preview",
        )
        .await?;
        ensure!(result["status"] == 201, "演示资料导入失败: {result}");
    }
    println!("{}", json!({"ready": true, "workspace": source}));
    let mut input = BufReader::new(tokio::io::stdin()).lines();
    while let Some(line) = input.next_line().await? {
        let result = async {
            let value: Value = serde_json::from_str(&line)?;
            let body: Vec<u8> = serde_json::from_value(value["body"].clone())?;
            let response = instance.handle(Request {
                method: value["method"].as_str().context("缺少请求方法")?.into(),
                path: value["path"].as_str().context("缺少请求路径")?.into(), query: None, headers: vec![], body,
            }, context("preview")).await?;
            Ok::<_, anyhow::Error>(json!({"status":response.status,"headers":response.headers.iter().map(|h| json!({"name":h.name,"value":h.value})).collect::<Vec<_>>(),"body":response.body}))
        }.await;
        match result {
            Ok(value) => println!("{value}"),
            Err(error) => println!("{}", json!({"error":format!("{error:#}")})),
        }
    }
    instance.lifecycle(Phase::Deactivate).await?;
    Ok(())
}
