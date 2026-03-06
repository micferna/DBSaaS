use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::UserRole;
use crate::repository::UserRepository;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: Uuid,
    pub email: String,
    pub role: UserRole,
    pub exp: usize,
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: Uuid,
    pub email: String,
    pub role: UserRole,
}

pub async fn auth_middleware(
    State(state): State<crate::AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::Unauthorized)?;

    let user = if let Some(token) = auth_header.strip_prefix("Bearer ") {
        // JWT auth
        let claims = decode::<Claims>(
            token,
            &DecodingKey::from_secret(state.config.jwt_secret.as_bytes()),
            &Validation::new(Algorithm::HS256),
        )?
        .claims;

        // Always verify role from DB — never trust the JWT claim alone
        let db_user = UserRepository::find_by_id(&state.db, claims.sub)
            .await?
            .ok_or(AppError::Unauthorized)?;

        AuthUser {
            id: db_user.id,
            email: db_user.email,
            role: db_user.role,
        }
    } else if let Some(api_key) = auth_header.strip_prefix("ApiKey ") {
        // API key auth
        let db_user = UserRepository::find_by_api_key(&state.db, api_key)
            .await?
            .ok_or(AppError::Unauthorized)?;

        AuthUser {
            id: db_user.id,
            email: db_user.email,
            role: db_user.role,
        }
    } else {
        return Err(AppError::Unauthorized);
    };

    request.extensions_mut().insert(user);
    Ok(next.run(request).await)
}

pub fn create_token(user_id: Uuid, email: &str, role: &UserRole, secret: &str) -> Result<String, AppError> {
    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(24))
        .unwrap()
        .timestamp() as usize;

    let claims = Claims {
        sub: user_id,
        email: email.to_string(),
        role: role.clone(),
        exp: expiration,
    };

    jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(AppError::Jwt)
}
