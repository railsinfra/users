# Users Microservice

Rust microservice for identity, businesses, environments, sessions, API keys, and related platform auth flows for the Rails Financial API.

## Overview

This service is part of the Rails Financial API open-source project and is designed for:

- Business and environment provisioning with role-based users
- JWT access and refresh sessions with secure refresh handling
- Business-scoped API keys (hashed at rest; one-time plaintext reveal on create)
- Optional email flows (password reset, beta notifications) via Resend
- gRPC surface for platform services alongside the public HTTP API

Core capabilities:

- Business registration and admin authentication
- User lifecycle data tied to businesses and environments
- API key administration for server-to-server and dashboard flows
- Rate-limited authentication-related HTTP routes

## Open Source Project

This microservice is maintained as part of an open-source repository.

- Monorepo overview: [`README.md`](../../../README.md)
- Contribution guide: [`CONTRIBUTING.md`](CONTRIBUTING.md)
- Code of conduct: [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)
- Security policy: [`SECURITY.md`](SECURITY.md)
- License: [`LICENSE`](LICENSE)

## Technology Stack

- Rust
- Axum (HTTP)
- Tokio
- Tonic (gRPC server and clients)
- SQLx (PostgreSQL)
- JSON Web Tokens (`jsonwebtoken`), HMAC API key hashing
- Serde, UUID, Chrono, Tracing
- Optional Sentry and Resend integrations

## Architecture

### Service design

- Stateless HTTP API using REST conventions under `/api/v1`
- gRPC server on `GRPC_PORT` (default `50051`) for internal callers; gRPC client to Accounts for downstream calls
- Modules under `src/routes/` for HTTP handlers, plus `config`, `db`, `auth`, `grpc`, `email`, and shared error types
- Layered middleware: correlation ID for `/api/*`, optional internal token gate on sensitive routes, client-key rate limits on auth-tier routes

### Performance and reliability

- Async I/O and pooled PostgreSQL access
- Indexed paths for common lookups (see migrations)
- Migrations run at startup; certain version drift scenarios are logged and skipped when connecting to shared or legacy databases (see below)

## Data Model

Core tables (evolving with migrations):

- `businesses`, `environments`
- `users`, `user_sessions`
- `api_keys` (hashed key material; status and revocation timestamps)
- `password_reset_tokens`, `beta_applications` (and related constraints)

Schema intent:

- UUID primary keys and foreign keys across tenant boundaries
- Enumerated string statuses where appropriate
- Audit timestamps on core entities
- Unique constraints where required (for example email deduplication rules per migration set)

## HTTP API Surface

### Correlation ID

For `/api/...` routes, include:

- `x-correlation-id: <string>`

If omitted, the service may generate one and attach it to the response.

### Environment selection

Authenticated routes that resolve tenant context typically require one of:

- `x-environment-id: <uuid>` (preferred), or
- `x-environment: <string>` (for example sandbox vs production labels), depending on the handler

### Authentication

Protected routes accept either:

- `authorization: Bearer <jwt>` (dashboard-style usage), or
- `x-api-key: <plaintext key>` (server-to-server; key is verified via `API_KEY_HASH_SECRET`)

API keys are business-scoped and treated as highly privileged for the owning business. Use the same `API_KEY_HASH_SECRET` on every replica that verifies keys.

### Sensitive routes and internal tokens

When `INTERNAL_SERVICE_TOKEN_ALLOWLIST` is non-empty, these routes require a matching `x-internal-service-token`:

- `POST /api/v1/auth/login`
- `POST /api/v1/business/register`

If the allowlist env var is unset or empty, that check is disabled (use explicit allowlists in shared environments and **require** a non-empty allowlist in `ENVIRONMENT=production` at process startup).

### Auth-tier rate limits

Login, password reset, beta apply, and related routes behind `auth_rate_limit_middleware` share a per-client-key limiter. Tune with:

- `USERS_AUTH_RATE_LIMIT_WINDOW_SECONDS` (default `60`)
- `USERS_AUTH_RATE_LIMIT_MAX` (default `10`)

Optional trusted proxy list for client IP extraction:

- `USERS_TRUSTED_PROXY_IPS` — comma-separated IPs that may terminate `x-forwarded-for`

### Endpoints

**Health**

- `GET /health` — Liveness / readiness style check

**Business**

- `POST /api/v1/business/register` — Register a business (internal token when allowlist configured)

**Auth**

- `POST /api/v1/auth/login` — Issue tokens (internal token when allowlist configured)
- `POST /api/v1/auth/refresh` — Refresh access token
- `POST /api/v1/auth/revoke` — Revoke refresh token
- `POST /api/v1/auth/password-reset/request` — Request password reset email (when Resend configured)
- `POST /api/v1/auth/password-reset/reset` — Complete password reset

**Beta**

- `POST /api/v1/beta/apply` — Beta application intake

**API keys (JWT or active API key)**

- `POST /api/v1/api-keys` — Create key; plaintext returned once
- `GET /api/v1/api-keys` — List keys for the environment
- `POST /api/v1/api-keys/:api_key_id/revoke` — Revoke a key

**Profile**

- `GET /api/v1/me` — Current user, business, and environment summary

### API key lifecycle notes

- Plaintext API keys are never stored; only a hash is persisted.
- Revoked keys remain rows with `status='revoked'` and `revoked_at` set.
- Rotate by creating a new key and revoking the old one if a secret is lost.

## Project Layout

```text
src/api/users/
├── src/
│   ├── main.rs
│   ├── config.rs
│   ├── db.rs
│   ├── auth.rs
│   ├── grpc.rs
│   ├── grpc_server.rs
│   ├── email.rs
│   ├── error.rs
│   └── routes/
├── migrations/
├── proto/
├── Cargo.toml
└── README.md
```

## Local Setup

### Prerequisites

- Rust stable toolchain
- PostgreSQL (local Docker or managed provider)
- SQLx CLI for manual migrations if you prefer not relying on startup migration only: `cargo install sqlx-cli`

### PostgreSQL (example)

```bash
# macOS
brew install postgresql@14
brew services start postgresql@14

# Or Docker
docker run --name postgres-users -e POSTGRES_PASSWORD=postgres -e POSTGRES_DB=users -p 5432:5432 -d postgres:14
```

Create the database and apply migrations:

```bash
createdb users
# Or: docker exec -i postgres-users psql -U postgres -c "CREATE DATABASE users;"

export DATABASE_URL="postgresql://localhost:5432/users"
sqlx migrate run

# Or run the initial SQL directly:
# psql "$DATABASE_URL" < migrations/20260119000100_init_users_service.sql
```

### Environment variables

Create `.env` in `src/api/users/` (see also [`.env.dev.example`](.env.dev.example)):

```env
# PostgreSQL — adjust user, password, host, port, database, sslmode for your environment.
DATABASE_URL=postgresql://USER:PASSWORD@HOST:PORT/DATABASE?sslmode=require

# HTTP bind: use SERVER_ADDR or HOST + PORT
SERVER_ADDR=0.0.0.0:8080
# HOST=0.0.0.0
# PORT=8080

GRPC_PORT=50051
ACCOUNTS_GRPC_URL=http://127.0.0.1:50052

RUST_LOG=info
ENVIRONMENT=development

# Required at startup (no insecure defaults). Example: openssl rand -hex 32
JWT_SECRET=replace_me
API_KEY_HASH_SECRET=replace_me_different_from_jwt

# Comma-separated. In production this must be non-empty or the process exits at startup.
INTERNAL_SERVICE_TOKEN_ALLOWLIST=

SENTRY_DSN=

FRONTEND_BASE_URL=http://localhost:5173

# Resend (password reset, beta mail). Optional in dev; password reset emails are skipped without a key.
RESEND_API_KEY=
RESEND_FROM_EMAIL=noreply@example.com
RESEND_FROM_NAME=Rails Financial Infrastructure
RESEND_BASE_URL=https://api.resend.com
RESEND_BETA_NOTIFICATION_EMAIL=beta@example.com
```

### Migration startup behavior

The service runs embedded SQLx migrations on boot. Some migration errors (for example missing or mismatched versions on shared databases) are logged and ignored so operators can still start against legacy schemas. Prefer a dedicated database and clean migration history for local development and new deployments.

### Build and run

```bash
cargo build --release
cargo run --release
```

Default HTTP URL: `http://localhost:8080` (or the host/port you configured).

## Development Workflow

- Run tests: `cargo test`
- Format: `cargo fmt`
- Lint: `cargo clippy`
- Add migration: `sqlx migrate add <name>`

## Performance Targets

Throughput and latency goals depend on deployment shape (CPU, connection pool size, and PostgreSQL tier). Size connection pools and instance counts against expected login, refresh, and API-key verification rates.

## Security and Compliance Baseline

- Required secrets at startup (`JWT_SECRET`, `API_KEY_HASH_SECRET`); no insecure defaults
- Production requires non-empty `INTERNAL_SERVICE_TOKEN_ALLOWLIST` for sensitive entrypoints
- Parameterized SQL, structured logging, and rate limiting on sensitive HTTP routes
- API keys stored as hashes; scrub sensitive headers in Sentry `before_send` when DSN is configured

## Observability and Error Tracking

- Structured tracing on request lifecycle (correlation ID, method, path, status, duration).
- Optional Sentry for unexpected failures; avoid treating validation or expected business-rule outcomes as crash-level events.
- Include service, environment, route, and correlation identifiers in telemetry; never log secrets or raw API keys.

## Maintainer-only SQL

`get_production_users.sql` is an operational helper for querying production-shaped data when you already have authorized database access. It is **not** required to run the service. Do not run it against production without your organization’s data-access policy.

## Troubleshooting

- **Startup hangs or DB errors:** confirm PostgreSQL is reachable (`pg_isready`, `psql`), and that `DATABASE_URL` points at an existing database.
- **Port in use:** change `SERVER_ADDR` / `PORT` or stop the conflicting process (`lsof -i :8080`).
- **Verbose diagnostics:** `RUST_LOG=debug cargo run`

### Quick manual check

```bash
curl http://localhost:8080/api/v1/business/register \
  -H "x-correlation-id: local-test-1" \
  -H "x-internal-service-token: replace_me" \
  -H "Content-Type: application/json" \
  -d '{"name":"Test Business","admin_email":"admin@test.com","admin_password":"password123"}'
```

(Adjust headers to match your `INTERNAL_SERVICE_TOKEN_ALLOWLIST` when enforcement is enabled.)

## Open Questions

- Long-term split between HTTP and gRPC for user provisioning and SDK-facing flows
- Hardening and policy for `INTERNAL_SERVICE_TOKEN_ALLOWLIST` rotation across services
- Deeper integration patterns with Accounts/Ledger for organization and money-movement workflows
