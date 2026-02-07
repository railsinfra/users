use axum::{Json, extract::State};
use uuid::Uuid;
use chrono::Utc;
use crate::error::{AppError, DUPLICATE_EMAIL_MESSAGE};
use crate::routes::{AppState, user};
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
    let admin_email_normalized = user::normalize_email(&payload.admin_email);
    if admin_email_normalized.is_empty() {
        return Err(AppError::BadRequest("Admin email is required.".to_string()));
    }

    // Application-level check before insert (defense in depth with DB constraint)
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)"
    )
    .bind(&admin_email_normalized)
    .fetch_one(&state.db)
    .await
    .map_err(|_| AppError::Internal)?;
    if exists {
        return Err(AppError::Conflict(DUPLICATE_EMAIL_MESSAGE.to_string()));
    }

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
    .bind(&admin_email_normalized)
    .bind(&password_hash)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        if let Some(db_err) = e.as_database_error() {
            if db_err.message().contains("unique_email") {
                return AppError::Conflict(DUPLICATE_EMAIL_MESSAGE.to_string());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::DUPLICATE_EMAIL_MESSAGE;
    use crate::grpc::GrpcClients;
    use sqlx::postgres::PgPoolOptions;

    async fn test_pool() -> Option<sqlx::PgPool> {
        let database_url = std::env::var("DATABASE_URL").ok()?;
        let pool = PgPoolOptions::new()
            .max_connections(2)
            .connect(&database_url)
            .await
            .ok()?;
        sqlx::migrate!("./migrations").run(&pool).await.ok()?;
        Some(pool)
    }

    fn unique_business_payload(admin_email: &str) -> RegisterBusinessRequest {
        RegisterBusinessRequest {
            name: format!("Biz {}", Uuid::new_v4()),
            website: None,
            admin_first_name: "Admin".to_string(),
            admin_last_name: "User".to_string(),
            admin_email: admin_email.to_string(),
            admin_password: "password123!".to_string(),
        }
    }

    #[tokio::test]
    async fn register_business_with_unique_email_succeeds() {
        let pool = match test_pool().await {
            Some(p) => p,
            None => {
                eprintln!("DATABASE_URL not set; skipping register_business integration test.");
                return;
            }
        };
        let state = AppState {
            db: pool.clone(),
            grpc: GrpcClients {
                accounts_client: None,
            },
            email: None,
        };
        let email = format!("unique+{}@example.com", Uuid::new_v4());
        let payload = unique_business_payload(&email);
        let result = register_business(State(state), Json(payload)).await;
        assert!(result.is_ok(), "Unique email registration should succeed: {:?}", result.err());
    }

    #[tokio::test]
    async fn register_business_duplicate_email_fails_with_conflict() {
        let pool = match test_pool().await {
            Some(p) => p,
            None => {
                eprintln!("DATABASE_URL not set; skipping duplicate email integration test.");
                return;
            }
        };
        let email = format!("dup+{}@example.com", Uuid::new_v4());
        let payload1 = unique_business_payload(&email);
        let state1 = AppState {
            db: pool.clone(),
            grpc: GrpcClients { accounts_client: None },
            email: None,
        };
        let _ = register_business(State(state1), Json(payload1)).await.expect("first register must succeed");
        let payload2 = unique_business_payload(&email);
        let state2 = AppState {
            db: pool.clone(),
            grpc: GrpcClients { accounts_client: None },
            email: None,
        };
        let result = register_business(State(state2), Json(payload2)).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        if let AppError::Conflict(msg) = err {
            assert_eq!(msg, DUPLICATE_EMAIL_MESSAGE, "Error message should be stable and user-friendly");
        } else {
            panic!("Expected Conflict, got {:?}", err);
        }
    }

    #[tokio::test]
    async fn register_business_case_insensitive_duplicate_blocked() {
        let pool = match test_pool().await {
            Some(p) => p,
            None => {
                eprintln!("DATABASE_URL not set; skipping case-insensitive duplicate test.");
                return;
            }
        };
        let base = format!("case+{}@example.com", Uuid::new_v4());
        let payload1 = unique_business_payload(&base);
        let state1 = AppState {
            db: pool.clone(),
            grpc: GrpcClients { accounts_client: None },
            email: None,
        };
        let _ = register_business(State(state1), Json(payload1)).await.expect("first register must succeed");
        let payload2 = unique_business_payload(&base.to_uppercase());
        let state2 = AppState {
            db: pool.clone(),
            grpc: GrpcClients { accounts_client: None },
            email: None,
        };
        let result = register_business(State(state2), Json(payload2)).await;
        assert!(result.is_err(), "Case-insensitive duplicate should be blocked");
        let err = result.unwrap_err();
        assert!(matches!(err, AppError::Conflict(_)), "Expected Conflict for case variant: {:?}", err);
    }
}
