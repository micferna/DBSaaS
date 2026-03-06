use axum::{extract::Request, middleware::Next, response::Response};

use crate::error::AppError;
use crate::middleware::auth::AuthUser;
use crate::models::UserRole;

pub async fn admin_middleware(request: Request, next: Next) -> Result<Response, AppError> {
    let user = request
        .extensions()
        .get::<AuthUser>()
        .ok_or(AppError::Unauthorized)?;

    if user.role != UserRole::Admin {
        return Err(AppError::Forbidden);
    }

    Ok(next.run(request).await)
}
