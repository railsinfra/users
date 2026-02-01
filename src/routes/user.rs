use axum::{Json, extract::{State, Query}, http::HeaderMap};
use uuid::Uuid;
use crate::{error::AppError};
use crate::routes::AppState;
use crate::auth::AuthContext;
use serde::{Deserialize, Serialize};
use argon2::{Argon2, PasswordHasher};
use argon2::password_hash::{SaltString, rand_core::OsRng};
use sqlx::Row;

#[derive(Deserialize)]
pub struct CreateUserRequest {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub password: String
}

#[derive(Serialize)]
pub struct CreateUserResponse {
    pub user_id: Uuid,
    pub status: String
}

#[derive(Serialize)]
pub struct MeUser {
    pub id: Uuid,
    pub business_id: Uuid,
    pub environment_id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub role: String,
    pub status: String,
}

#[derive(Serialize)]
pub struct MeBusiness {
    pub id: Uuid,
    pub name: String,
    pub website: Option<String>,
    pub status: String,
}

#[derive(Serialize)]
pub struct MeEnvironment {
    pub id: Uuid,
    pub business_id: Uuid,
    pub r#type: String,
    pub status: String,
}

#[derive(Serialize)]
pub struct MeResponse {
    pub user: MeUser,
    pub business: MeBusiness,
    pub environment: MeEnvironment,
}

#[derive(Serialize)]
pub struct ListUser {
    pub id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub role: String,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
pub struct ListUsersQuery {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

#[derive(Serialize)]
pub struct PaginationMeta {
    pub page: u32,
    pub per_page: u32,
    pub total_count: i64,
    pub total_pages: u32,
}

#[derive(Serialize)]
pub struct ListUsersResponse {
    pub data: Vec<ListUser>,
    pub pagination: PaginationMeta,
}

pub async fn me(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<Json<MeResponse>, AppError> {
    let user_id = ctx.user_id.ok_or(AppError::Forbidden)?;
    
    // First, try to find user in the requested environment
    let user_row = sqlx::query(
        "SELECT id, business_id, environment_id, first_name, last_name, email, role, status FROM users WHERE id = $1 AND environment_id = $2 AND status = 'active'"
    )
    .bind(&user_id)
    .bind(&ctx.environment_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| AppError::Internal)?;
    
    // If user doesn't exist in requested environment, find them in any environment for the same business
    // This allows users to access both sandbox and production even if they only have a user record in one
    let user_row = if let Some(row) = user_row {
        Some(row)
    } else {
        // Find user in any environment for the same business
        let any_user_row = sqlx::query(
            "SELECT id, business_id, environment_id, first_name, last_name, email, role, status FROM users WHERE id = $1 AND business_id = $2 AND status = 'active' LIMIT 1"
        )
        .bind(&user_id)
        .bind(&ctx.business_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| AppError::Internal)?;
        
        // Verify that the requested environment_id belongs to the same business
        if any_user_row.is_some() {
            let env_check = sqlx::query(
                "SELECT 1 FROM environments WHERE id = $1 AND business_id = $2 AND status = 'active'"
            )
            .bind(&ctx.environment_id)
            .bind(&ctx.business_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|_| AppError::Internal)?;
            
            if env_check.is_none() {
                return Err(AppError::Forbidden);
            }
        }
        
        any_user_row
    };
    
    let user_row = user_row.ok_or(AppError::Forbidden)?;

    let user = MeUser {
        id: user_row.get("id"),
        business_id: user_row.get("business_id"),
        environment_id: user_row.get("environment_id"),
        first_name: user_row.get("first_name"),
        last_name: user_row.get("last_name"),
        email: user_row.get("email"),
        role: user_row.get("role"),
        status: user_row.get("status"),
    };

    let env_row = sqlx::query(
        "SELECT id, business_id, type, status FROM environments WHERE id = $1 AND status = 'active'"
    )
    .bind(&ctx.environment_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| AppError::Internal)?
    .ok_or(AppError::BadRequest("Invalid environment_id".to_string()))?;

    let environment = MeEnvironment {
        id: env_row.get("id"),
        business_id: env_row.get("business_id"),
        r#type: env_row.get("type"),
        status: env_row.get("status"),
    };

    let business_row = sqlx::query(
        "SELECT id, name, website, status FROM businesses WHERE id = $1 AND status = 'active'"
    )
    .bind(&user.business_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| AppError::Internal)?
    .ok_or(AppError::BadRequest("Invalid business_id".to_string()))?;

    let business = MeBusiness {
        id: business_row.get("id"),
        name: business_row.get("name"),
        website: business_row.get("website"),
        status: business_row.get("status"),
    };

    Ok(Json(MeResponse {
        user,
        business,
        environment,
    }))
}

pub async fn create_user(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(payload): Json<CreateUserRequest>
) -> Result<Json<CreateUserResponse>, AppError> {
    let user_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    let salt = SaltString::generate(&mut OsRng);
    let password_hash = Argon2::default()
        .hash_password(payload.password.as_bytes(), &salt)
        .map_err(|_| AppError::Internal)?
        .to_string();

    let environment_id = ctx.environment_id;
    let business_id = ctx.business_id;

    if ctx.api_key_id.is_none() {
        let request_user_id = ctx.user_id.ok_or(AppError::Forbidden)?;
        let role_row = sqlx::query(
            "SELECT role FROM users WHERE id = $1 AND environment_id = $2 AND status = 'active'"
        )
        .bind(&request_user_id)
        .bind(&environment_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| AppError::Internal)?
        .ok_or(AppError::Forbidden)?;

        let role: String = role_row.get("role");
        if role != "admin" {
            return Err(AppError::Forbidden);
        }
    }

    let created_by_user_id = ctx.user_id;
    let created_by_api_key_id = ctx.api_key_id;

    let env_row = sqlx::query(
        "SELECT type FROM environments WHERE id = $1 AND business_id = $2 AND status = 'active'",
    )
    .bind(&environment_id)
    .bind(&business_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| AppError::Internal)?
    .ok_or(AppError::Forbidden)?;

    let environment_type: String = env_row.get("type");

    sqlx::query(
        "INSERT INTO users (id, business_id, environment_id, first_name, last_name, email, password_hash, role, status, created_at, updated_at, created_by_user_id, created_by_api_key_id) VALUES ($1, $2, $3, $4, $5, $6, $7, 'user', 'active', $8, $8, $9, $10)"
    )
    .bind(&user_id)
    .bind(&business_id)
    .bind(&environment_id)
    .bind(&payload.first_name)
    .bind(&payload.last_name)
    .bind(&payload.email)
    .bind(&password_hash)
    .bind(&now)
    .bind(&created_by_user_id)
    .bind(&created_by_api_key_id)
    .execute(&state.db)
    .await
    .map_err(|e| {
        if let Some(db_err) = e.as_database_error() {
            if db_err.message().contains("unique_email") {
                return AppError::BadRequest("Email already exists".to_string());
            }
        }
        AppError::Internal
    })?;

    // Create account synchronously via gRPC
    // Customer accounts require admin_user_id - use the admin who created this user
    let admin_user_id = created_by_user_id.ok_or_else(|| {
        tracing::error!("Cannot create account: customer accounts require an admin_user_id, but created_by_user_id is None");
        AppError::BadRequest("Customer accounts require an admin user. User must be created by an admin.".to_string())
    })?;

    match state.grpc.create_default_account(
        business_id,
        &environment_type,
        user_id,
        admin_user_id,
        "USD",
    ).await {
        Ok((account_id, account_number)) => {
            tracing::info!(
                "Created account {} ({}) for user {} via gRPC",
                account_id,
                account_number,
                user_id
            );
        }
        Err(e) => {
            tracing::error!(
                "Failed to create account for user {} via gRPC: {}",
                user_id,
                e
            );
            // Return error with more details - account creation is critical
            // Check if client is initialized
            let error_msg = if state.grpc.accounts_client.is_some() {
                format!("gRPC call failed: {}. Check Accounts service logs.", e)
            } else {
                format!("gRPC client not initialized: {}. Ensure Accounts service is running and ACCOUNTS_GRPC_URL is correct.", e)
            };
            return Err(AppError::BadRequest(error_msg));
        }
    }

    Ok(Json(CreateUserResponse {
        user_id,
        status: "active".to_string(),
    }))
}

pub async fn list_users(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(query): Query<ListUsersQuery>,
    headers: HeaderMap,
) -> Result<Json<ListUsersResponse>, AppError> {
    let environment_id = ctx.environment_id;
    let business_id = ctx.business_id;

    // Only admins can list users (or API keys which are treated as admin-level)
    if ctx.api_key_id.is_none() {
        let user_id = ctx.user_id.ok_or(AppError::Forbidden)?;
        
        // Check if user is admin in the requested environment
        let role_row = sqlx::query(
            "SELECT role FROM users WHERE id = $1 AND environment_id = $2 AND status = 'active'"
        )
        .bind(&user_id)
        .bind(&environment_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| AppError::Internal)?;

        // If user doesn't exist in requested environment, check if they're admin in any environment for the same business
        // This allows admins to access both sandbox and production even if they only have a user record in one
        let is_admin = if let Some(row) = role_row {
            let role: String = row.get("role");
            role == "admin"
        } else {
            // User doesn't exist in requested environment, check if they're admin in any environment for this business
            let admin_check = sqlx::query(
                "SELECT 1 FROM users WHERE id = $1 AND business_id = $2 AND role = 'admin' AND status = 'active' LIMIT 1"
            )
            .bind(&user_id)
            .bind(&business_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|_| AppError::Internal)?;
            
            admin_check.is_some()
        };

        if !is_admin {
            return Err(AppError::Forbidden);
        }
    }

    // Get X-Environment header (sandbox/production) to filter by environment type
    // This allows filtering by environment type in addition to environment_id
    let environment_type: Option<String> = headers
        .get("x-environment")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_lowercase())
        .filter(|s| s == "sandbox" || s == "production");

    // Parse and validate pagination params with defaults
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(10).min(100).max(1);
    let offset = (page - 1) * per_page;

    let environment_type_filter = environment_type.as_deref();

    // Build query with optional environment type filter
    // If X-Environment header is provided, filter by both environment_id AND environment.type
    // This ensures we only get users from the correct environment type (sandbox/production)
    let (count_query, list_query) = list_users_queries(environment_type_filter);

    // Get total count
    let count_row = if let Some(env_type) = environment_type_filter {
        sqlx::query(count_query)
            .bind(&business_id)
            .bind(&environment_id)
            .bind(env_type)
            .fetch_one(&state.db)
            .await
            .map_err(|_| AppError::Internal)?
    } else {
        sqlx::query(count_query)
            .bind(&business_id)
            .bind(&environment_id)
            .fetch_one(&state.db)
            .await
            .map_err(|_| AppError::Internal)?
    };

    let total_count: i64 = count_row.get("count");
    let total_pages = ((total_count as f64) / (per_page as f64)).ceil() as u32;

    // Fetch paginated results with deterministic ordering
    let rows = if let Some(env_type) = environment_type_filter {
        sqlx::query(list_query)
            .bind(&business_id)
            .bind(&environment_id)
            .bind(env_type)
            .bind(per_page as i64)
            .bind(offset as i64)
            .fetch_all(&state.db)
            .await
            .map_err(|_| AppError::Internal)?
    } else {
        sqlx::query(list_query)
            .bind(&business_id)
            .bind(&environment_id)
            .bind(per_page as i64)
            .bind(offset as i64)
            .fetch_all(&state.db)
            .await
            .map_err(|_| AppError::Internal)?
    };

    let users: Vec<ListUser> = rows
        .into_iter()
        .map(|row| ListUser {
            id: row.get("id"),
            first_name: row.get("first_name"),
            last_name: row.get("last_name"),
            email: row.get("email"),
            role: row.get("role"),
            status: row.get("status"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
        .collect();

    Ok(Json(ListUsersResponse {
        data: users,
        pagination: PaginationMeta {
            page,
            per_page,
            total_count,
            total_pages,
        },
    }))
}

fn list_users_queries(environment_type: Option<&str>) -> (&'static str, &'static str) {
    if environment_type.is_some() {
        // Filter by business_id, environment_id, AND environment.type
        (
            "SELECT COUNT(*) as count FROM users u 
             INNER JOIN environments e ON u.environment_id = e.id 
             WHERE u.business_id = $1 AND u.environment_id = $2 AND e.type = $3 AND u.status = 'active'",
            "SELECT u.id, u.first_name, u.last_name, u.email, u.role, u.status, u.created_at, u.updated_at 
             FROM users u 
             INNER JOIN environments e ON u.environment_id = e.id 
             WHERE u.business_id = $1 AND u.environment_id = $2 AND e.type = $3 AND u.status = 'active' 
             ORDER BY u.created_at DESC, u.id DESC LIMIT $4 OFFSET $5",
        )
    } else {
        // Fallback to original behavior: filter by environment_id only
        (
            "SELECT COUNT(*) as count FROM users WHERE business_id = $1 AND environment_id = $2 AND status = 'active'",
            "SELECT id, first_name, last_name, email, role, status, created_at, updated_at FROM users WHERE business_id = $1 AND environment_id = $2 AND status = 'active' ORDER BY created_at DESC, id DESC LIMIT $3 OFFSET $4",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::list_users_queries;

    #[test]
    fn list_users_queries_with_env_type_includes_environment_join() {
        let (count_query, list_query) = list_users_queries(Some("sandbox"));
        assert!(count_query.contains("INNER JOIN environments"));
        assert!(count_query.contains("e.type = $3"));
        assert!(list_query.contains("INNER JOIN environments"));
        assert!(list_query.contains("e.type = $3"));
    }

    #[test]
    fn list_users_queries_without_env_type_omits_environment_join() {
        let (count_query, list_query) = list_users_queries(None);
        assert!(!count_query.contains("INNER JOIN environments"));
        assert!(!count_query.contains("e.type = $3"));
        assert!(!list_query.contains("INNER JOIN environments"));
        assert!(!list_query.contains("e.type = $3"));
    }
}
