pub use crate::model::MemoryContext;
use axum::{extract::FromRequestParts, http::request::Parts};

impl<S> FromRequestParts<S> for MemoryContext
where
    S: Send + Sync,
{
    type Rejection = axum::http::StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<MemoryContext>()
            .cloned()
            .ok_or(axum::http::StatusCode::UNAUTHORIZED)
    }
}
