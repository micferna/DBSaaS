use bollard::Docker;

use crate::error::{AppError, AppResult};

pub fn create_docker_client(host: &Option<String>) -> AppResult<Docker> {
    match host {
        Some(h) if h.starts_with("tcp://") => Docker::connect_with_http(h, 120, bollard::API_DEFAULT_VERSION)
            .map_err(|e| AppError::Internal(format!("Docker connection failed: {e}"))),
        _ => Docker::connect_with_local_defaults()
            .map_err(|e| AppError::Internal(format!("Docker connection failed: {e}"))),
    }
}
