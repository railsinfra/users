use axum::{Json, extract::State};
use uuid::Uuid;
use chrono::Utc;
use crate::{error::AppError};
use crate::routes::AppState;
use serde::{Deserialize, Serialize};
use argon2::{Argon2, PasswordHasher};
use argon2::password_hash::{SaltString, rand_core::OsRng};

#[derive(Deserialize)]
pub struct RegisterBusinessRequest {
    pub name: String,
    pub website: Option<String>,
    pub admin_first_name: String,
    pub admin_last_name: String,
    pub admin_email: String,
    pub admin_password: String
}

#[derive(Serialize)]
pub struct RegisterBusinessResponse {
    pub business_id: Uuid,
    pub admin_user_id: Uuid,
    pub environments: Vec<EnvironmentInfo>
}

#[derive(Serialize)]
pub struct EnvironmentInfo {
    pub id: Uuid,
    pub r#type: String
}

pub async fn register_business(
    State(state): State<AppState>,
    Json(payload): Json<RegisterBusinessRequest>
) -> Result<Json<RegisterBusinessResponse>, AppError> {
    let mut tx = state.db.begin().await.map_err(|_| AppError::Internal)?;

    // 1. Create business
    let business_id = Uuid::new_v4();
    let now = Utc::now();
    sqlx::query(
        "INSERT INTO businesses (id, name, website, status, created_at, updated_at) VALUES ($1, $2, $3, 'active', $4, $4)"
    )
    .bind(&business_id)
    .bind(&payload.name)
    .bind(&payload.website)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(|_| AppError::Internal)?;

    // 2. Create environments (sandbox, production)
    let sandbox_env_id = Uuid::new_v4();
    let prod_env_id = Uuid::new_v4();
    for (env_id, env_type) in [(sandbox_env_id, "sandbox"), (prod_env_id, "production")] {
        sqlx::query(
            "INSERT INTO environments (id, business_id, type, status, created_at, updated_at) VALUES ($1, $2, $3, 'active', $4, $4)"
        )
        .bind(&env_id)
        .bind(&business_id)
        .bind(env_type)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::Internal)?;
    }

    // 3. Create admin user (in both environments)
    let admin_user_id = Uuid::new_v4();
    let salt = SaltString::generate(&mut OsRng);
    let password_hash = Argon2::default()
        .hash_password(payload.admin_password.as_bytes(), &salt)
        .map_err(|_| AppError::Internal)?
        .to_string();
    // Create the default admin in sandbox
    sqlx::query(
        "INSERT INTO users (id, business_id, environment_id, first_name, last_name, email, password_hash, role, status, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, 'admin', 'active', $8, $8)"
    )
    .bind(&admin_user_id)
    .bind(&business_id)
    .bind(&sandbox_env_id)
    .bind(&payload.admin_first_name)
    .bind(&payload.admin_last_name)
    .bind(&payload.admin_email)
    .bind(&password_hash)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        if let Some(db_err) = e.as_database_error() {
            if db_err.message().contains("unique_email") {
                return AppError::BadRequest("Email already exists".to_string());
            }
        }
        AppError::Internal
    })?;

    tx.commit().await.map_err(|_| AppError::Internal)?;

    Ok(Json(RegisterBusinessResponse {
        business_id,
        admin_user_id,
        environments: vec![
            EnvironmentInfo { id: sandbox_env_id, r#type: "sandbox".to_string() },
            EnvironmentInfo { id: prod_env_id, r#type: "production".to_string() },
        ],
    }))
}
