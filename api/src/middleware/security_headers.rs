use axum::{extract::Request, middleware::Next, response::Response};

pub async fn security_headers_middleware(
    request: Request,
    next: Next,
) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    headers.insert("x-content-type-options", "nosniff".parse().unwrap());
    headers.insert("x-frame-options", "DENY".parse().unwrap());
    headers.insert("x-xss-protection", "1; mode=block".parse().unwrap());
    headers.insert("referrer-policy", "strict-origin-when-cross-origin".parse().unwrap());
    headers.insert(
        "strict-transport-security",
        "max-age=31536000; includeSubDomains".parse().unwrap(),
    );
    headers.insert(
        "content-security-policy",
        "default-src 'none'; frame-ancestors 'none'".parse().unwrap(),
    );
    headers.insert("permissions-policy", "camera=(), microphone=(), geolocation=()".parse().unwrap());

    // Remove server identification
    headers.remove("server");

    response
}
