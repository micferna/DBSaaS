use axum::{
    extract::{Multipart, Path, State},
    Extension,
};
use crate::extract::Json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::middleware::auth::AuthUser;
use crate::models::MigrationRecord;
use crate::repository::DatabaseRepository;
use crate::services::migration::MigrationService;
use crate::utils::crypto::decrypt_string;
use crate::AppState;

pub async fn upload_migration(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(db_id): Path<Uuid>,
    mut multipart: Multipart,
) -> AppResult<Json<MigrationRecord>> {
    // Verify ownership
    let db_inst = DatabaseRepository::find_by_id(&state.db, db_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database not found".to_string()))?;

    if db_inst.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    if db_inst.database_name.is_none() {
        return Err(AppError::BadRequest(
            "Migrations only supported for PostgreSQL databases".to_string(),
        ));
    }

    let mut filename = String::new();
    let mut sql_content = String::new();

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        AppError::BadRequest(format!("Multipart error: {e}"))
    })? {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            filename = field
                .file_name()
                .unwrap_or("migration.sql")
                .to_string();

            if !filename.ends_with(".sql") {
                return Err(AppError::BadRequest("Only .sql files accepted".to_string()));
            }

            let bytes = field
                .bytes()
                .await
                .map_err(|e| AppError::BadRequest(format!("Failed to read file: {e}")))?;

            if bytes.len() > 10 * 1024 * 1024 {
                return Err(AppError::BadRequest("File too large (max 10MB)".to_string()));
            }

            sql_content = String::from_utf8(bytes.to_vec())
                .map_err(|_| AppError::BadRequest("File must be valid UTF-8".to_string()))?;
        }
    }

    if sql_content.is_empty() {
        return Err(AppError::BadRequest("No SQL file provided".to_string()));
    }

    // Compute checksum
    let mut hasher = Sha256::new();
    hasher.update(sql_content.as_bytes());
    let checksum = format!("{:x}", hasher.finalize());

    // Execute migration
    let password = decrypt_string(&db_inst.password_encrypted, &state.config.encryption_key)?;

    MigrationService::execute_migration(
        &db_inst.host,
        db_inst.port,
        &db_inst.username,
        &password,
        db_inst.database_name.as_deref().unwrap(),
        &sql_content,
    )
    .await?;

    // Record migration
    let record =
        MigrationService::record_migration(&state.db, db_id, &filename, &checksum).await?;

    Ok(Json(record))
}

pub async fn list_migrations(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(db_id): Path<Uuid>,
) -> AppResult<Json<Vec<MigrationRecord>>> {
    let db_inst = DatabaseRepository::find_by_id(&state.db, db_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database not found".to_string()))?;

    if db_inst.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    let records = MigrationService::list_migrations(&state.db, db_id).await?;
    Ok(Json(records))
}
