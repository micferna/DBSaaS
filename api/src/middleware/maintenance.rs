use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;

pub async fn maintenance_middleware(
    req: Request<Body>,
    next: Next,
) -> Response {
    // Extract maintenance_mode from extensions
    let maintenance = req
        .extensions()
        .get::<Arc<RwLock<bool>>>()
        .cloned();

    if let Some(maintenance_mode) = maintenance {
        let is_maintenance = *maintenance_mode.read().await;
        if is_maintenance {
            let method = req.method().clone();
            let path = req.uri().path().to_string();

            // Allow GET requests and admin endpoints
            let is_safe = method == Method::GET || path.starts_with("/api/admin");

            if !is_safe {
                let body = json!({ "error": "Platform is under maintenance" });
                return (StatusCode::SERVICE_UNAVAILABLE, axum::Json(body)).into_response();
            }
        }
    }

    next.run(req).await
}
