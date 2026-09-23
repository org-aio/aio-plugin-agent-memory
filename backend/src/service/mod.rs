mod access;
mod classifier;
pub mod context;
pub mod graph;
pub mod intake;
pub mod isolate;
pub mod nodes;
pub mod queue;
pub mod routing;
pub mod secrets;
pub mod spaces;
pub mod store;

pub use access::{MemoryError, Result};
pub use context::MemoryContext;
pub use graph::{activation, context, graph, recall, route, search, visibility};
pub use intake::{capture, get as source_get, import, list as source_list, original, retry};
pub use nodes::{create_edge, delete, delete_edge, edges, get, revisions, rollback, sources};
pub use queue::{claim, fail, proposal, resolve, source_proposal, submit};
pub use secrets::{grant, list as secrets_list, reveal};
pub use spaces::{add_member, list, members, remove_member};

use crate::{configuration::RuntimeConfig, cryptography::Cryptography};
use anyhow::Result as AnyResult;
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::sync::Arc;

#[derive(Clone)]
pub struct MemoryService {
    pub(crate) pool: PgPool,
    pub(crate) crypto: Cryptography,
}

impl MemoryService {
    pub fn new(pool: PgPool, crypto: Cryptography) -> Self {
        Self { pool, crypto }
    }
}

pub async fn build(config: RuntimeConfig) -> AnyResult<Arc<MemoryService>> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;
    let crypto = Cryptography::new(config.broker_socket, config.broker_token);
    Ok(Arc::new(MemoryService::new(pool, crypto)))
}
