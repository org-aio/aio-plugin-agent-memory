use crate::{model::MemoryContext, service::MemoryService};
use axum::{
    Router,
    extract::{DefaultBodyLimit, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use std::sync::Arc;
use subtle::ConstantTimeEq;

#[derive(Clone)]
pub struct Ingress {
    pub token: String,
    pub tenant: Option<String>,
}

async fn authenticate(
    State(ingress): State<Ingress>,
    mut request: Request,
    next: Next,
) -> Response {
    let token = request
        .headers()
        .get("x-aio-token")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    if token.as_bytes().ct_eq(ingress.token.as_bytes()).unwrap_u8() != 1 {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    fn read(request: &Request, name: &str) -> Option<String> {
        request
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .filter(|value| !value.is_empty() && value.len() <= 128)
            .map(str::to_owned)
    }
    let (Some(tenant_id), Some(user_id)) = (
        read(&request, "x-aio-tenant-id"),
        read(&request, "x-aio-user-id"),
    ) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if ingress
        .tenant
        .as_ref()
        .is_some_and(|bound| bound != &tenant_id)
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let context_id = read(&request, "x-aio-context");
    request.extensions_mut().insert(MemoryContext {
        tenant_id,
        user_id,
        context_id,
    });
    next.run(request).await
}

pub fn router(service: Arc<MemoryService>, ingress: Ingress) -> Router {
    let application = Router::new()
        .route(
            "/spaces",
            get(crate::service::spaces::list).post(crate::service::spaces::create),
        )
        .route("/spaces/{id}", put(crate::service::spaces::update))
        .route(
            "/spaces/{id}/members",
            get(crate::service::spaces::members).post(crate::service::spaces::add_member),
        )
        .route(
            "/spaces/{id}/members/{user}",
            axum::routing::delete(crate::service::spaces::remove_member),
        )
        .route("/capture", post(crate::service::intake::capture))
        .route("/sources", get(crate::service::intake::list))
        .route("/sources/{id}", get(crate::service::intake::get))
        .route(
            "/sources/{id}/original",
            post(crate::service::intake::original),
        )
        .route("/sources/{id}/retry", post(crate::service::intake::retry))
        .route(
            "/sources/{id}/proposal",
            get(crate::service::queue::source_proposal),
        )
        .route(
            "/sources/{id}/resolve",
            post(crate::service::queue::resolve),
        )
        .route("/secrets", get(crate::service::secrets::list))
        .route(
            "/secrets/{id}/reveal",
            post(crate::service::secrets::reveal),
        )
        .route("/secrets/{id}/grants", put(crate::service::secrets::grant))
        .route("/tasks/claim", post(crate::service::queue::claim))
        .route("/tasks/{id}/submit", post(crate::service::queue::submit))
        .route("/tasks/{id}/fail", post(crate::service::queue::fail))
        .route("/tasks/{id}/proposal", get(crate::service::queue::proposal))
        .route("/graph", get(crate::service::graph::graph))
        .route("/search", post(crate::service::graph::search))
        .route("/recall", post(crate::service::graph::recall))
        .route("/route", post(crate::service::graph::route))
        .route("/activation", post(crate::service::graph::activation))
        .route("/visibility", post(crate::service::graph::visibility))
        .route("/nodes", post(crate::service::nodes::create))
        .route(
            "/nodes/{id}",
            get(crate::service::nodes::get)
                .put(crate::service::nodes::update)
                .delete(crate::service::nodes::delete),
        )
        .route("/nodes/{id}/edges", get(crate::service::nodes::edges))
        .route(
            "/nodes/{id}/revisions",
            get(crate::service::nodes::revisions),
        )
        .route(
            "/nodes/{id}/rollback",
            post(crate::service::nodes::rollback),
        )
        .route("/nodes/{id}/sources", get(crate::service::nodes::sources))
        .route("/edges", post(crate::service::nodes::create_edge))
        .route(
            "/edges/{id}",
            axum::routing::delete(crate::service::nodes::delete_edge),
        )
        .route("/import", post(crate::service::intake::import))
        .route("/context", post(crate::service::graph::context))
        .layer(DefaultBodyLimit::max(512 * 1024))
        .layer(middleware::from_fn_with_state(ingress, authenticate))
        .with_state(service);
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/aio/describe", get(crate::hosting::describe))
        .merge(application)
}
