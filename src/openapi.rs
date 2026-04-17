//! OpenAPI (utoipa) for HTTP routes. Stubs are documentation-only; handlers live under `routes/`.

#![allow(dead_code)]

use utoipa::OpenApi;

use crate::routes::{
    apikey::{ApiKeyInfo, CreateApiKeyRequest, CreateApiKeyResponse},
    auth::{
        EnvironmentInfo as LoginEnvironmentInfo, LoginRequest, LoginResponse, RefreshTokenRequest,
        RefreshTokenResponse, RevokeTokenRequest, RevokeTokenResponse,
    },
    beta::{BetaApplicationRequest, BetaApplicationResponse},
    business::{EnvironmentInfo as RegisterEnvironmentInfo, RegisterBusinessRequest, RegisterBusinessResponse},
    password_reset::{
        RequestPasswordResetRequest, RequestPasswordResetResponse, ResetPasswordRequest,
        ResetPasswordResponse,
    },
    user::{MeBusiness, MeEnvironment, MeResponse, MeUser},
};

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct UsersHealthBody {
    pub status: String,
    pub service: String,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct UsersErrorBody {
    pub error: String,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Users API",
        version = "0.1.0",
        description = "Authentication, businesses, API keys, and session APIs. Correlation id: send or receive `x-correlation-id` on `/api/*` routes."
    ),
    paths(
        health_doc,
        register_business_doc,
        login_doc,
        refresh_token_doc,
        revoke_token_doc,
        request_password_reset_doc,
        reset_password_doc,
        apply_beta_doc,
        create_api_key_doc,
        list_api_keys_doc,
        revoke_api_key_doc,
        me_doc,
    ),
    components(schemas(
        UsersHealthBody,
        UsersErrorBody,
        RegisterBusinessRequest,
        RegisterBusinessResponse,
        RegisterEnvironmentInfo,
        LoginRequest,
        LoginResponse,
        LoginEnvironmentInfo,
        RefreshTokenRequest,
        RefreshTokenResponse,
        RevokeTokenRequest,
        RevokeTokenResponse,
        RequestPasswordResetRequest,
        RequestPasswordResetResponse,
        ResetPasswordRequest,
        ResetPasswordResponse,
        BetaApplicationRequest,
        BetaApplicationResponse,
        CreateApiKeyRequest,
        CreateApiKeyResponse,
        ApiKeyInfo,
        MeResponse,
        MeUser,
        MeBusiness,
        MeEnvironment,
    )),
    tags(
        (name = "health", description = "Liveness"),
        (name = "auth", description = "Sessions and tokens"),
        (name = "business", description = "Registration"),
        (name = "password-reset", description = "Password reset"),
        (name = "beta", description = "Beta intake"),
        (name = "api-keys", description = "API keys"),
        (name = "user", description = "Current user context"),
    ),
)]
pub struct ApiDoc;

#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses((status = 200, description = "OK", body = UsersHealthBody))
)]
pub fn health_doc() {}

#[utoipa::path(
    post,
    path = "/api/v1/business/register",
    tag = "business",
    request_body = RegisterBusinessRequest,
    params(
        ("X-Internal-Service-Token" = Option<String>, Header, description = "Required when INTERNAL_SERVICE_TOKEN_ALLOWLIST is set"),
    ),
    responses(
        (status = 200, description = "Registered", body = RegisterBusinessResponse),
        (status = 400, body = UsersErrorBody),
        (status = 403, body = UsersErrorBody),
        (status = 409, body = UsersErrorBody),
    )
)]
pub fn register_business_doc() {}

#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    tag = "auth",
    request_body = LoginRequest,
    params(
        ("X-Internal-Service-Token" = Option<String>, Header, description = "Required when INTERNAL_SERVICE_TOKEN_ALLOWLIST is set"),
    ),
    responses(
        (status = 200, description = "OK", body = LoginResponse),
        (status = 401, body = UsersErrorBody),
        (status = 429, body = UsersErrorBody),
    )
)]
pub fn login_doc() {}

#[utoipa::path(
    post,
    path = "/api/v1/auth/refresh",
    tag = "auth",
    request_body = RefreshTokenRequest,
    responses(
        (status = 200, description = "OK", body = RefreshTokenResponse),
        (status = 401, body = UsersErrorBody),
    )
)]
pub fn refresh_token_doc() {}

#[utoipa::path(
    post,
    path = "/api/v1/auth/revoke",
    tag = "auth",
    request_body = RevokeTokenRequest,
    responses(
        (status = 200, description = "OK", body = RevokeTokenResponse),
        (status = 401, body = UsersErrorBody),
    )
)]
pub fn revoke_token_doc() {}

#[utoipa::path(
    post,
    path = "/api/v1/auth/password-reset/request",
    tag = "password-reset",
    request_body = RequestPasswordResetRequest,
    responses(
        (status = 200, description = "OK", body = RequestPasswordResetResponse),
        (status = 429, body = UsersErrorBody),
    )
)]
pub fn request_password_reset_doc() {}

#[utoipa::path(
    post,
    path = "/api/v1/auth/password-reset/reset",
    tag = "password-reset",
    request_body = ResetPasswordRequest,
    responses(
        (status = 200, description = "OK", body = ResetPasswordResponse),
        (status = 400, body = UsersErrorBody),
        (status = 429, body = UsersErrorBody),
    )
)]
pub fn reset_password_doc() {}

#[utoipa::path(
    post,
    path = "/api/v1/beta/apply",
    tag = "beta",
    request_body = BetaApplicationRequest,
    responses(
        (status = 200, description = "OK", body = BetaApplicationResponse),
        (status = 400, body = UsersErrorBody),
        (status = 409, body = UsersErrorBody),
        (status = 429, body = UsersErrorBody),
    )
)]
pub fn apply_beta_doc() {}

#[utoipa::path(
    post,
    path = "/api/v1/api-keys",
    tag = "api-keys",
    request_body = CreateApiKeyRequest,
    params(
        ("Authorization" = Option<String>, Header, description = "Bearer JWT"),
        ("X-API-Key" = Option<String>, Header, description = "Alternative to JWT"),
        ("X-Environment-Id" = Option<uuid::Uuid>, Header, description = "Target environment UUID"),
        ("X-Environment" = Option<String>, Header, description = "sandbox or production when using API key without environment id"),
    ),
    responses(
        (status = 200, description = "Created", body = CreateApiKeyResponse),
        (status = 403, body = UsersErrorBody),
        (status = 401, body = UsersErrorBody),
    )
)]
pub fn create_api_key_doc() {}

#[utoipa::path(
    get,
    path = "/api/v1/api-keys",
    tag = "api-keys",
    params(
        ("Authorization" = Option<String>, Header, description = "Bearer JWT"),
        ("X-API-Key" = Option<String>, Header, description = "Alternative to JWT"),
        ("X-Environment-Id" = Option<uuid::Uuid>, Header, description = "Environment UUID"),
        ("X-Environment" = Option<String>, Header, description = "sandbox or production with API key"),
    ),
    responses(
        (status = 200, description = "OK", body = [ApiKeyInfo]),
        (status = 401, body = UsersErrorBody),
        (status = 403, body = UsersErrorBody),
    )
)]
pub fn list_api_keys_doc() {}

#[utoipa::path(
    post,
    path = "/api/v1/api-keys/{api_key_id}/revoke",
    tag = "api-keys",
    params(
        ("api_key_id" = uuid::Uuid, Path, description = "API key id"),
        ("Authorization" = Option<String>, Header, description = "Bearer JWT"),
        ("X-API-Key" = Option<String>, Header, description = "Alternative to JWT"),
        ("X-Environment-Id" = Option<uuid::Uuid>, Header, description = "Environment UUID"),
        ("X-Environment" = Option<String>, Header, description = "sandbox or production with API key"),
    ),
    responses(
        (status = 200, description = "Revoked (same shape as create; `key` is empty)", body = CreateApiKeyResponse),
        (status = 400, body = UsersErrorBody),
        (status = 401, body = UsersErrorBody),
        (status = 403, body = UsersErrorBody),
    )
)]
pub fn revoke_api_key_doc() {}

#[utoipa::path(
    get,
    path = "/api/v1/me",
    tag = "user",
    params(
        ("Authorization" = Option<String>, Header, description = "Bearer JWT"),
        ("X-API-Key" = Option<String>, Header, description = "Alternative to JWT"),
        ("X-Environment-Id" = Option<uuid::Uuid>, Header, description = "Environment UUID"),
        ("X-Environment" = Option<String>, Header, description = "sandbox or production with API key"),
    ),
    responses(
        (status = 200, description = "OK", body = MeResponse),
        (status = 401, body = UsersErrorBody),
        (status = 403, body = UsersErrorBody),
    )
)]
pub fn me_doc() {}
