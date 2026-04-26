# ADR 0001 — Rust + Axum for the backend

**Status:** Accepted (2026-04-24)

## Context

We need a backend that proxies Elasticsearch traffic, authenticates sessions,
streams uploads, and runs a background cleanup task. The team picked Rust.

Options considered:

- **Rust + Axum** — modern async stack, Tokio ecosystem, strong type safety
  for the proxy's validation logic.
- Python + FastAPI — faster to prototype, weaker guarantees around streaming
  and memory for large uploads.
- Node.js + Fastify — familiar to frontend devs, but a mismatch with the
  language preference.

## Decision

Use **Rust 1.85+** with **Axum 0.7** on Tokio. `reqwest` for the ES HTTP
client (a thin proxy doesn't need a typed ES client and we *want* to show
learners the raw JSON). `sqlx` for SQLite. `jsonwebtoken` for session tokens.

## Consequences

- **Positive:** compile-time guarantees around route handlers, cheap async
  streaming for multipart uploads, small static binary for deployment.
- **Negative:** steeper onboarding; iteration slower than Python. Mitigated
  by keeping the surface small and the module boundaries explicit.
- Build artifacts live in `backend/target/` — excluded from version control.
