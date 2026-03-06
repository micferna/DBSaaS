use axum::{extract::State, Extension};
use crate::extract::Json;
use validator::Validate;

use crate::error::{AppError, AppResult};
use crate::middleware::auth::{create_token, AuthUser};
use crate::models::{AuthResponse, CreateUserRequest, LoginRequest, UserInfo, UserRole};
use crate::repository::{InvitationRepository, UserRepository};
use crate::utils::crypto::{generate_api_key, hash_password, verify_password};
use crate::AppState;

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<CreateUserRequest>,
) -> AppResult<Json<AuthResponse>> {
    req.validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    // Check registration settings (runtime-mutable)
    let reg_enabled = *state.registration_enabled.read().await;
    if !reg_enabled {
        let code = req
            .invitation_code
            .as_deref()
            .ok_or_else(|| AppError::BadRequest("Registration is disabled. Invitation code required.".to_string()))?;

        let invitation = InvitationRepository::find_by_code(&state.db, code)
            .await?
            .ok_or_else(|| AppError::BadRequest("Invalid invitation code".to_string()))?;

        if invitation.use_count >= invitation.max_uses {
            return Err(AppError::BadRequest("Invitation code exhausted".to_string()));
        }

        if let Some(expires) = invitation.expires_at {
            if expires < chrono::Utc::now() {
                return Err(AppError::BadRequest("Invitation code expired".to_string()));
            }
        }
    }

    // Check existing user
    if UserRepository::find_by_email(&state.db, &req.email)
        .await?
        .is_some()
    {
        return Err(AppError::Conflict("Email already registered".to_string()));
    }

    let password_hash = hash_password(&req.password)?;
    let user = UserRepository::create(&state.db, &req.email, &password_hash, &UserRole::User).await?;

    // Use invitation code if provided
    if let Some(code) = &req.invitation_code {
        InvitationRepository::use_invitation(&state.db, code, user.id).await?;
    }

    let token = create_token(user.id, &user.email, &user.role, &state.config.jwt_secret)?;

    Ok(Json(AuthResponse {
        token,
        user: UserInfo::from(&user),
    }))
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> AppResult<Json<AuthResponse>> {
    let user = UserRepository::find_by_email(&state.db, &req.email)
        .await?
        .ok_or(AppError::Unauthorized)?;

    if !verify_password(&req.password, &user.password_hash)? {
        return Err(AppError::Unauthorized);
    }

    let token = create_token(user.id, &user.email, &user.role, &state.config.jwt_secret)?;

    Ok(Json(AuthResponse {
        token,
        user: UserInfo::from(&user),
    }))
}

pub async fn me(Extension(user): Extension<AuthUser>) -> AppResult<Json<UserInfo>> {
    Ok(Json(UserInfo {
        id: user.id,
        email: user.email,
        role: user.role,
    }))
}

pub async fn generate_api_key_handler(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> AppResult<Json<serde_json::Value>> {
    let api_key = generate_api_key();
    UserRepository::set_api_key(&state.db, user.id, &api_key).await?;

    Ok(Json(serde_json::json!({ "api_key": api_key })))
}
