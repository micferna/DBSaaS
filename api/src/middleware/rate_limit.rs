use axum::{extract::{ConnectInfo, Request}, middleware::Next, response::Response};
use governor::{DefaultKeyedRateLimiter, Quota, RateLimiter};
use std::net::{IpAddr, SocketAddr};
use std::num::NonZeroU32;
use std::sync::Arc;

use crate::error::AppError;

pub fn create_rate_limiter(requests_per_second: u32) -> Arc<DefaultKeyedRateLimiter<IpAddr>> {
    Arc::new(RateLimiter::keyed(Quota::per_second(
        NonZeroU32::new(requests_per_second).unwrap(),
    )))
}

/// Known trusted proxy IPs (internal Docker network IPs).
/// Only trust X-Forwarded-For from these addresses.
fn is_trusted_proxy(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private(),
        IpAddr::V6(v6) => v6.is_loopback(),
    }
}

/// Extract the client IP. Uses peer address as primary source.
/// Only trusts X-Forwarded-For/X-Real-Ip if the peer is a known proxy.
fn extract_client_ip(request: &Request) -> IpAddr {
    // Get the actual peer address from the connection
    let peer_ip = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ci| ci.0.ip());

    let peer = peer_ip.unwrap_or(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));

    // Only trust proxy headers if the connection comes from a trusted proxy
    if is_trusted_proxy(peer) {
        // Try X-Forwarded-For (set by Traefik/reverse proxies)
        if let Some(xff) = request.headers().get("x-forwarded-for") {
            if let Ok(xff_str) = xff.to_str() {
                if let Some(first_ip) = xff_str.split(',').next() {
                    if let Ok(ip) = first_ip.trim().parse::<IpAddr>() {
                        return ip;
                    }
                }
            }
        }

        // Try X-Real-Ip
        if let Some(xri) = request.headers().get("x-real-ip") {
            if let Ok(xri_str) = xri.to_str() {
                if let Ok(ip) = xri_str.trim().parse::<IpAddr>() {
                    return ip;
                }
            }
        }
    }

    peer
}

pub async fn rate_limit_middleware(
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let limiter = request
        .extensions()
        .get::<Arc<DefaultKeyedRateLimiter<IpAddr>>>()
        .cloned();

    if let Some(limiter) = limiter {
        let ip = extract_client_ip(&request);
        if limiter.check_key(&ip).is_err() {
            return Err(AppError::RateLimited);
        }
    }

    Ok(next.run(request).await)
}
