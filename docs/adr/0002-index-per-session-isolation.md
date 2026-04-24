# ADR 0002 — Isolation by index prefix on a shared cluster

**Status:** Accepted (2026-04-24)

## Context

We must isolate each learner's data from every other learner's data and wipe
it when their session ends. Candidate approaches:

1. **Shared Elasticsearch cluster, index-per-session** — every index is
   prefixed `session-<sid>-<name>`. Ownership checks in the proxy.
2. **Dedicated ES container per session** — strongest isolation, heaviest
   cost. ~30 s cold start on ordinary hardware; memory-hungry even idle.
3. **Elastic Cloud Serverless project per session** — managed tenancy, but
   ongoing per-tenant cost makes a free/educational tier impractical.

## Decision

Use approach 1: a single shared cluster with index-per-session naming.

The prefix is enforced by two helpers in `backend/src/es.rs`:

- `prefixed_index(sid, name) → "session-<sid>-<name>"` — the *only* way a
  handler constructs a full index name.
- `assert_owned(sid, full)` — belt-and-braces check before making the HTTP
  call. Rejects wildcards, commas, slashes, whitespace.

Cleanup is a single `DELETE /session-<sid>-*` per expired session.

## Consequences

- **Positive:** essentially zero per-session overhead. Works on a 2 GB VM.
- **Positive:** cleanup is one HTTP call per session and is idempotent.
- **Negative:** a bug in either helper could cross-session access. Mitigated
  by unit tests in `es.rs` and by reviewing any change to those functions as
  a security change.
- **Negative:** hot-spots on the shared cluster are visible to all users.
  Acceptable for a teaching app; not acceptable for PII.
- Revisiting this decision (e.g. to move to per-session containers) would
  require a new ADR and touches `es.rs`, `cleanup.rs`, and `docker-compose.yml`.
