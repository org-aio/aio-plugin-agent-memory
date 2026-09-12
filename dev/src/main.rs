mod configuration;
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
        .await
        .with_context(|| format!("Memory verification request: {method} {path}"))?;
    Ok(
        json!({"status": response.status, "body": if response.body.is_empty() { Value::Null } else { serde_json::from_slice::<Value>(&response.body)? }}),
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_target(false)
        .init();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("缺少插件目录")?;
    let verifying = std::env::args().any(|arg| arg == "--verify");
    let (source, keyring) = configuration::load(root, verifying)?;
    let provisioner =
        DatabaseProvisioner::connect(&std::env::var("AIO_TEST_DATABASE_URL")?).await?;
    let mut files: Vec<_> = std::fs::read_dir(root.join("backend/migrations"))?
        .collect::<std::io::Result<Vec<_>>>()?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "sql"))
        .collect();
    files.sort();
    let migrations = files
        .iter()
        .map(|path| {
            Ok((
                path.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read_to_string(path)?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let migration = migrations
        .iter()
        .map(|(_, sql)| sql.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let database = provisioner
        .install(&source, "preview", &migrations, &keyring)
        .await?;
    let scope = InvocationScope {
        source_id: source.clone(),
        revision: "development".into(),
        context: context("preview"),
        grants: CapabilityGrants {
            database: true,
            cryptography: true,
            ..Default::default()
        },
    };
    let resources = InvocationResources {
        database: Some(database),
        keyring: Some(std::sync::Arc::new(keyring)),
        services: Some(std::sync::Arc::new(configuration::DevelopmentServices)),
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
    if verifying {
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
            json!({"requestId":uuid::Uuid::new_v4(),"title":"记忆工作台设计记录", "text":source_text}),
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
            let mut request_context = context("preview");
            request_context.user_id = Some(value["userId"].as_str().unwrap_or("developer").into());
            request_context.session_id = Some(if value["worker"].as_bool()==Some(true) {"service:agent"} else {"preview-browser"}.into());
            let response = instance.handle(Request {
                method: value["method"].as_str().context("缺少请求方法")?.into(),
                path: value["path"].as_str().context("缺少请求路径")?.into(), query: value["query"].as_str().map(str::to_owned), headers: vec![], body,
            }, request_context).await?;
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
