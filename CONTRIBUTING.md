# Contributing

Thanks for contributing to `users`.

## Development setup

1. Fork and clone the repository.
2. Copy environment templates and configure local values:
   - `cp .env.example .env`
   - Set `JWT_SECRET` and `API_KEY_HASH_SECRET` to long random values (see README).
3. Start required local dependencies (PostgreSQL, Rust toolchain).
4. Run tests before opening a PR.

## Branch and PR workflow

- Create a branch from `main`.
- Keep changes scoped and small where possible.
- Open a PR with:
  - clear problem statement
  - implementation details
  - test evidence
  - migration/config notes (if any)

## Commit style

- Use conventional commits when possible:
  - `feat: ...`
  - `fix: ...`
  - `docs: ...`
  - `chore: ...`
  - `refactor: ...`
  - `test: ...`

## Testing expectations

- Add or update tests alongside functional changes.
- Validate both happy path and failure path for backend changes.

## Observability expectations

- Treat observability as part of done.
- Capture unexpected system failures through the monitoring layer (Sentry-backed).
- Add/update tracking for critical flows.
- Never include secrets or sensitive financial/personal data in logs, analytics, or error payloads.

## Security and secrets

- Never commit credentials, API keys, or private tokens.
- Use placeholders in docs and examples.
- If you discover a vulnerability, follow `SECURITY.md` instead of filing a public issue.

## Code of conduct

By participating, you agree to follow `CODE_OF_CONDUCT.md`.
