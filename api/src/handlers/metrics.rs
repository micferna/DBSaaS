use axum::response::IntoResponse;
use axum::http::header;

use crate::services::metrics::render_metrics;

pub async fn prometheus_metrics() -> impl IntoResponse {
    let body = render_metrics();
    (
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        body,
    )
}
