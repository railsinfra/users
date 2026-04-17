# Users Microservice (Rust)

Rust microservice for identity, businesses, environments, sessions, API keys, and platform authentication flows. This service is part of the Rails Financial API monorepo and is designed to be explicit about secrets, safe for multi-tenant workloads, and observable at the edge.

## Open Source Project

This service is maintained as part of the open-source repository:

- Project overview: [`README.md`](../../../README.md)
- Contribution guide: [`CONTRIBUTING.md`](CONTRIBUTING.md)
- Code of conduct: [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)
- Security policy: [`SECURITY.md`](SECURITY.md)
- License: [`LICENSE`](LICENSE)

## What This Service Does

- Registers businesses and environments and ties users to those tenants
- Issues and refreshes JWT access sessions with refresh-token lifecycle controls
- Issues business-scoped API keys (hashed at rest; plaintext shown once on create)
- Protects sensitive HTTP entrypoints with optional internal service tokens
- Rate-limits authentication-tier HTTP routes by client identity behind trusted proxies
- Exposes a gRPC server for platform services alongside the public HTTP API
- Sends optional email flows (password reset, beta notifications) when Resend is configured

## Architecture Overview

The service follows a conventional Rust layout:

- `src/main.rs` - process entry, tracing/Sentry wiring, migration run, HTTP + gRPC servers
- `src/routes/` - Axum HTTP handlers (`business`, `auth`, `apikey`, `user`, `beta`, `password_reset`, `health`)
- `src/auth.rs` - JWT and API-key extraction and validation
- `src/grpc.rs` / `src/grpc_server.rs` - gRPC client to Accounts and users gRPC surface
- `src/config.rs`, `src/db.rs`, `src/email.rs`, `src/error.rs` - configuration, SQLx pool, mail, errors
- `migrations/` - SQLx migrations applied at startup
- `proto/` - protobuf definitions consumed by tonic

## API Surface

### HTTP endpoints

Base URL defaults to `http://localhost:8080` (see `SERVER_ADDR` or `HOST` + `PORT`).

Public and session routes:

- `GET /health` - health check
- `POST /api/v1/business/register` - register a business
- `POST /api/v1/auth/refresh` - refresh access token
- `POST /api/v1/auth/revoke` - revoke refresh token

Auth-tier (rate-limited) routes:

- `POST /api/v1/auth/login` - login and issue tokens
- `POST /api/v1/auth/password-reset/request` - request password reset email (requires Resend)
- `POST /api/v1/auth/password-reset/reset` - complete password reset
- `POST /api/v1/beta/apply` - beta application intake

Protected (JWT or active API key) routes:

- `POST /api/v1/api-keys` - create API key (plaintext returned once)
- `GET /api/v1/api-keys` - list API keys for the environment
- `POST /api/v1/api-keys/:api_key_id/revoke` - revoke an API key
- `GET /api/v1/me` - current user, business, and environment summary

### Auth behavior

- All `/api/...` routes participate in correlation ID handling: send `x-correlation-id` or receive one on the response.
- Authenticated routes resolve tenant context using `x-environment-id` (UUID) and/or `x-environment` (`sandbox` / `production`) depending on the handler.
- Protected routes accept `Authorization: Bearer <jwt>` or `x-api-key: <plaintext>`. API keys are verified with `API_KEY_HASH_SECRET` (must match every validating replica).
- When `INTERNAL_SERVICE_TOKEN_ALLOWLIST` is non-empty, `POST /api/v1/auth/login` and `POST /api/v1/business/register` require `x-internal-service-token` matching the allowlist. In `ENVIRONMENT=production`, the allowlist must be non-empty at startup.
- Auth-tier routes honor `USERS_TRUSTED_PROXY_IPS` (comma-separated IPs) when deriving client keys for rate limiting, plus `USERS_AUTH_RATE_LIMIT_WINDOW_SECONDS` and `USERS_AUTH_RATE_LIMIT_MAX`.

### gRPC methods

Protobuf sources live under `proto/` (for example `users.proto`). The gRPC server listens on `GRPC_PORT` (default `50051`). Regenerate Rust stubs after proto changes using the project’s `build.rs` workflow.

## Local Setup

### Prerequisites

- Rust stable toolchain
- PostgreSQL 14+ (local or managed)
- Optional: `cargo install sqlx-cli` if you prefer running migrations separately from app startup

### Environment configuration

Use the provided example file:

```bash
cp .env.dev.example .env
```

Expected variables (non-exhaustive; see `.env.dev.example` for the full set):

- `DATABASE_URL` - Postgres connection string
- `SERVER_ADDR` or `HOST` + `PORT` - HTTP bind (defaults include `0.0.0.0:8080`)
- `GRPC_PORT` - gRPC server port (default `50051`)
- `ACCOUNTS_GRPC_URL` - Accounts service gRPC endpoint for downstream calls
- `RUST_LOG` - tracing filter (for example `info`)
- `ENVIRONMENT` - logical environment name (`development`, `production`, …)
- `JWT_SECRET` - required signing secret for JWTs
- `API_KEY_HASH_SECRET` - required secret for API key hashing (distinct from `JWT_SECRET`)
- `INTERNAL_SERVICE_TOKEN_ALLOWLIST` - optional comma-separated tokens; required non-empty in production
- `SENTRY_DSN` - optional error reporting DSN
- Resend-related variables - optional; password reset and beta mail are skipped without `RESEND_API_KEY`

For local development, use non-production credentials and local databases only.

If your PostgreSQL database does not exist yet, create one before running migrations:

```bash
createdb users || true
```

### Install and run

```bash
export DATABASE_URL="postgresql://localhost:5432/users"   # adjust to your DB
sqlx migrate run    # optional if you rely on startup migrations only
cargo build
cargo run
```

The service also runs embedded SQLx migrations on boot. Some migration version drift on shared databases is logged and ignored; prefer a clean dedicated database for local work.

## Testing

Run the Rust tests from this directory:

```bash
cargo test
```

Some integration tests require `DATABASE_URL` and will skip when it is unset. Set `JWT_SECRET` and `API_KEY_HASH_SECRET` when running tests that exercise crypto paths in CI or locally.

When changing auth, routing, or migration behavior, add or update tests in the same change.

## Operational Notes

- API key plaintext is never stored; only a derived hash is persisted. Rotate by creating a new key and revoking the old one.
- Revoked API keys remain rows with `status='revoked'` and `revoked_at` set.
- `get_production_users.sql` is a maintainer-only operational helper, not required to run the service; do not run it against production without your organization’s data-access policy.
- Core relational tables include `businesses`, `environments`, `users`, `user_sessions`, `api_keys`, and tables supporting password reset and beta applications (see `migrations/`).

## Observability and Error Tracking

- Emit structured logs for HTTP request lifecycle (correlation ID, method, path, status, duration).
- Capture unexpected system failures with Sentry when `SENTRY_DSN` is set; scrub `Authorization`, `X-Internal-Service-Token`, and `X-API-Key` in `before_send`.
- Include actionable context (service, environment, route, correlation ID), never secrets or raw API keys.
- Keep monitoring behavior consistent with repo-wide observability guidance.

## Deployment

- Service deployment config is in `railway.toml`.
- For production usage, ensure secure env var management, strict secret handling, and a non-empty `INTERNAL_SERVICE_TOKEN_ALLOWLIST`.
- Keep migrations and deployment changes in sync with this service release.

## Security Reporting

Please report vulnerabilities privately via [`SECURITY.md`](SECURITY.md). Do not open public issues for security disclosures.
