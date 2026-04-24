# ADR 0003 — SQLite for session metadata

**Status:** Accepted (2026-04-24)

## Context

We need to store a list of live sessions with their expiry timestamps. A
background task scans this list every 30 s to find expired rows and wipe the
corresponding ES indices.

The entire schema is one table with three columns. Options:

- **SQLite** via `sqlx` — embedded, transactional, zero infra.
- **Postgres** — heavier, overkill for one table, one instance.
- **Redis** with TTL + keyspace notifications — fast but adds a service and
  its durability story is weaker.
- **In-memory `HashMap`** — lost on restart, stranding ES indices.

## Decision

Use **SQLite** via `sqlx` with a single migration. The DB file lives on a
Docker volume (`backend-data`). Schema:

```sql
CREATE TABLE sessions (
    sid         TEXT PRIMARY KEY,
    created_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at  DATETIME NOT NULL
);
CREATE INDEX idx_sessions_expires_at ON sessions (expires_at);
```

## Consequences

- **Positive:** one fewer service to run, back up, or monitor.
- **Positive:** `sqlx` gives compile-time-checked queries and typed rows.
- **Negative:** single writer. For our workload (one INSERT per new session,
  one periodic SELECT + a few DELETEs) this is fine.
- If we ever need to shard the backend horizontally, revisit this decision —
  Postgres would be the obvious swap.
