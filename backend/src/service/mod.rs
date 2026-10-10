mod access;
mod clarification;
mod classifier;
pub mod context;
pub mod graph;
pub mod intake;
pub mod isolate;
pub mod nodes;
pub mod queue;
pub mod routing;
pub mod secrets;
mod source_dedup;
pub mod source_edit;
pub mod spaces;
pub mod ssh;
pub mod store;

pub use access::{MemoryError, Result};
pub use context::MemoryContext;
pub use graph::{activation, context, graph, recall, route, search, visibility};
pub use intake::{
    attachment, capture, get as source_get, import, list as source_list, original, retry,
};
pub use nodes::{create_edge, delete, delete_edge, edges, get, revisions, rollback, sources};
pub use queue::{claim, fail, proposal, resolve, source_proposal, submit};
pub use secrets::{grant, list as secrets_list, reveal};
pub use spaces::{add_member, list, members, remove_member};
pub use ssh::{
    apply as ssh_apply, create as ssh_create, delete as ssh_delete, devices as ssh_devices,
    list as ssh_list, update as ssh_update, verify as ssh_verify,
};

use crate::{configuration::RuntimeConfig, cryptography::Cryptography};
use anyhow::{Context, Result as AnyResult};
use reqwest::Client;
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct MemoryService {
    pub(crate) pool: PgPool,
    pub(crate) crypto: Cryptography,
    pub(crate) broker_token: Option<String>,
    pub(crate) broker: Client,
}

impl MemoryService {
    pub fn new(
        pool: PgPool,
        crypto: Cryptography,
        broker_socket: Option<PathBuf>,
        broker_token: Option<String>,
    ) -> AnyResult<Self> {
        let mut builder = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(std::time::Duration::from_secs(10));
        if let Some(socket) = broker_socket {
            builder = builder.unix_socket(socket);
        }
        Ok(Self {
            pool,
            crypto,
            broker_token,
            broker: builder.build().context("无法初始化宿主连接")?,
        })
    }
}

pub async fn build(config: RuntimeConfig) -> AnyResult<Arc<MemoryService>> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;
    let crypto = Cryptography::new(config.broker_socket.clone(), config.broker_token.clone());
    Ok(Arc::new(MemoryService::new(
        pool,
        crypto,
        config.broker_socket,
        config.broker_token,
    )?))
}
