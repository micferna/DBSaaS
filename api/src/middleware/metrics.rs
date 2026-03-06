use axum::{
    body::Body,
    http::Request,
    middleware::Next,
    response::Response,
};
use std::time::Instant;

use crate::services::metrics::{HTTP_REQUEST_COUNT, HTTP_REQUEST_DURATION};

pub async fn metrics_middleware(req: Request<Body>, next: Next) -> Response {
    let method = req.method().to_string();
    let path = normalize_path(req.uri().path());
    let start = Instant::now();

    let response = next.run(req).await;

    let status = response.status().as_u16().to_string();
    let duration = start.elapsed().as_secs_f64();

    HTTP_REQUEST_COUNT
        .with_label_values(&[&method, &path, &status])
        .inc();
    HTTP_REQUEST_DURATION
        .with_label_values(&[&method, &path])
        .observe(duration);

    response
}

/// Normalize paths by replacing UUIDs with {id} to avoid cardinality explosion
fn normalize_path(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    let normalized: Vec<String> = parts
        .iter()
        .map(|part| {
            if uuid::Uuid::parse_str(part).is_ok() {
                "{id}".to_string()
            } else {
                part.to_string()
            }
        })
        .collect();
    normalized.join("/")
}
