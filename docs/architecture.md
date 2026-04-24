# Architecture

## Big picture

```
              ┌───────────────────────────┐
   Browser ─▶ │  React SPA (Vite, :5173)  │
              └──────────────┬────────────┘
                             │  /api/*, cookies: el_session
                             ▼
              ┌───────────────────────────┐
              │  Rust API (Axum, :8080)   │
              │  ─ auth (JWT cookie)      │
              │  ─ es proxy (prefix gate) │
              │  ─ cleanup task (30s)     │
              └────┬───────────────┬──────┘
                   │               │
              SQL  │               │ HTTP
                   ▼               ▼
         ┌─────────────────┐  ┌──────────────────────┐
         │  SQLite         │  │  Elasticsearch       │
         │  sessions table │  │  indices:            │
         │  (sid, exp)     │  │  session-<sid>-*     │
         └─────────────────┘  └──────────────────────┘
```

The SPA talks only to the Rust API. The Rust API is the *only* component with
access to Elasticsearch credentials and network reach.

## Components

### 1. React SPA (`frontend/`)

Single-page app built with Vite. Three panes:

- **Workspace** — upload (`DatasetUpload`) and index list (`IndexList`).
- **Query** — Monaco editor (`QueryEditor`) emitting `POST /api/query`.
- **Response** — pretty-printed ES response (`ResultsPane`).

State lives in a small zustand store (`src/store.ts`). All network calls go
through `src/api.ts`, which always sends `credentials: "include"` so the
session cookie rides along.

### 2. Rust API (`backend/`)

Axum on Tokio, single binary. Modules:

- `config` — env → `Config`.
- `state` — `AppState { cfg, pool, es }` cloned into every handler.
- `auth` — JWT encode/decode + `AuthSession` extractor (validates cookie *and*
  a live DB row).
- `db` — `sqlx::SqlitePool` + three helpers (`insert`, `get`, `list_expired`,
  `delete`).
- `es` — thin `reqwest` wrapper. Public functions always take `sid` and a
  user-facing index name; they resolve to `session-<sid>-<name>` internally.
- `routes/` — one file per resource.
- `cleanup` — `tokio::spawn`ed task, 30-second interval.
- `error` — single `AppError` → JSON response.

### 3. Elasticsearch

Single-node, security disabled (dev only). Exposed to Docker network, not the
public internet. Accessed exclusively via the backend.

### 4. SQLite

One file, one table. Used because it's embedded, transactional, and ideal for
the small amount of metadata we need. If the file is lost we only lose live
sessions — not historical data.

## Data model

### SQLite: `sessions`

| Column | Type | Notes |
|---|---|---|
| `sid` | TEXT PK | ULID, lowercase, 26 chars |
| `created_at` | DATETIME | default `CURRENT_TIMESTAMP` |
| `expires_at` | DATETIME | enforced by `AuthSession` and cleanup |

### Elasticsearch: index naming

```
session-<sid>-<user-chosen-name>
          │           │
          │           └─ 1..=64 chars of [a-z0-9_-], can't start with `_`
          └─ 26-char ULID in lowercase
```

Example: `session-01htgf…abc-movies`.

The prefix is the *single source of truth* for ownership. Any function that
touches ES by name goes through `es::prefixed_index` or `es::assert_owned`.

## Sequence diagrams

### Start session

```
Browser ──POST /api/sessions──▶ API
                                 │
                                 ├─ sid = ULID
                                 ├─ expires_at = now + TTL
                                 ├─ INSERT INTO sessions
                                 └─ JWT{sid,exp} → Set-Cookie
                                 ◀──201 + {session_id, expires_at}
```

### Upload dataset

```
Browser ──POST /api/datasets?index=movies&format=ndjson (multipart) ─▶ API
                                                                       │
                                                                       ├─ AuthSession (401 if expired)
                                                                       ├─ full = prefixed_index(sid,"movies")
                                                                       ├─ PUT  /<full>          ← create or reuse
                                                                       ├─ parse NDJSON → chunks of 1000
                                                                       └─ POST /_bulk          ← per chunk
                                                                       ◀─ 200 {index, doc_count}
```

### Run query

```
Browser ──POST /api/query {index:"movies", body:{…}}──▶ API
                                                         │
                                                         ├─ AuthSession
                                                         ├─ reject_unsafe(body)
                                                         ├─ full = prefixed_index(sid, "movies")
                                                         ├─ assert_owned(sid, full)
                                                         └─ POST /<full>/_search
                                                         ◀─ 200 { raw ES response }
```

### Session expiration (background)

```
every 30s ─▶ cleanup::sweep
              │
              ├─ SELECT … FROM sessions WHERE expires_at < now
              │
              for each expired row:
              ├─ DELETE /session-<sid>-*         (404 OK)
              └─ DELETE FROM sessions WHERE sid=?
```

## Trust boundaries

| Boundary | Trust in | Enforced by |
|---|---|---|
| Browser → API | anything — assume hostile | `AuthSession`, `es::assert_owned`, `reject_unsafe`, multipart limits |
| API → SQLite | trusted (same process) | parameterised queries via sqlx |
| API → Elasticsearch | trusted network link | `FRONTEND_ORIGIN` CORS + allowlisted endpoints |

## Future content pipeline (out of scope for MVP)

When tutorials or auto-graded challenges land, the natural shape is:

- A `challenges` table in SQLite (id, title, markdown, expected query, seed
  dataset path).
- A grader endpoint that runs the user's query and the reference query, then
  compares hit IDs.
- Seed datasets indexed into the session at start, then wiped like any other.
