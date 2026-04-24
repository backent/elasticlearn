# CLAUDE.md — ElasticLearn onboarding brief

> Read this first. It is the shortest path from "I just opened this repo" to
> "I can safely change things." For deeper topics, follow the links to `docs/`.

## What this is

A sandboxed Elasticsearch learning playground. Developers open the site, get
an anonymous time-boxed session (default 60 min), upload a dataset, and run
ES queries against it. When the session expires, every index the user
created is deleted by a background task.

**Fixed design decisions (do not reopen without an ADR):**

| Topic | Decision |
|---|---|
| Backend | Rust + Axum on Tokio |
| Frontend | React + Vite + Monaco (TypeScript) |
| Isolation | Shared ES cluster, index-per-session with `session-<sid>-<name>` prefix |
| Auth | Anonymous, signed JWT in an httpOnly+SameSite=Strict cookie |
| Session store | SQLite (one table, `sessions`) |
| Day-one content | Free-form playground only — no tutorials or challenges yet |

## How to run it

```bash
cp .env.example .env                      # set JWT_SECRET
docker compose up -d elasticsearch        # ES on :9200
cd backend  && cargo run                  # API on :8080
cd frontend && npm install && npm run dev # SPA on :5173
```

Or the whole stack in Docker: `docker compose up --build`.

## Where things live

```
elasticlearn/
├── backend/
│   ├── Cargo.toml
│   ├── Dockerfile
│   ├── migrations/
│   │   └── 20260424000000_sessions.sql
│   └── src/
│       ├── main.rs          ← Axum bootstrap + route table
│       ├── config.rs        ← env → Config struct
│       ├── state.rs         ← AppState (cfg + pool + ES client)
│       ├── auth.rs          ← JWT cookie + AuthSession extractor
│       ├── db.rs            ← SQLite sessions table
│       ├── es.rs            ← HTTP client + index-prefix helpers
│       ├── cleanup.rs       ← background task, 30 s interval
│       ├── analytics.rs     ← track() → events table (usage analytics)
│       ├── error.rs         ← AppError → JSON response
│       └── routes/
│           ├── sessions.rs  ← POST/GET/DELETE /api/sessions…
│           ├── datasets.rs  ← POST /api/datasets
│           ├── query.rs     ← POST /api/query, GET /api/indices, /mapping
│           └── admin.rs     ← GET /api/admin/stats (ADMIN_TOKEN gated)
├── frontend/
│   ├── package.json
│   ├── vite.config.ts       ← proxies /api → :8080
│   └── src/
│       ├── main.tsx · App.tsx · styles.css
│       ├── api.ts           ← typed fetch wrappers
│       ├── store.ts         ← zustand
│       ├── pages/Playground.tsx
│       └── components/
│           ├── SessionTimer.tsx
│           ├── DatasetUpload.tsx
│           ├── IndexList.tsx
│           ├── QueryEditor.tsx   (Monaco)
│           └── ResultsPane.tsx
├── docker-compose.yml
├── samples/movies.ndjson
└── docs/
    ├── architecture.md · api.md · security.md · operations.md
    └── adr/0001…0004*.md
```

## Invariants that must not break

- The **browser never talks to Elasticsearch directly**. Only the Rust backend
  holds ES credentials and opens connections.
- **Every index created by a session is prefixed `session-<sid>-`.** Users
  send the short name (`movies`); the server resolves it. The resolver lives
  in `backend/src/es.rs::prefixed_index`.
- **`/api/query` validates index ownership** with `es::assert_owned` before
  forwarding. Cross-session access must return 403.
- **Scripted queries are rejected** — `reject_unsafe` in `routes/query.rs`
  walks the body and blocks `script`, `script_fields`, `runtime_mappings`.
- **Cleanup is idempotent.** `DELETE /<pattern>` returns success even when the
  indices are already gone; the DB row is then removed.
- **Upload caps are server-enforced**, not trusted from the client:
  `MAX_UPLOAD_BYTES` and `MAX_DOCS_PER_INDEX`.
- **Analytics writes never fail a user request.** `analytics::track` logs and
  swallows errors — treat it as fire-and-forget.
- **Event `meta` must not contain query bodies or document contents.** Only
  structural data (index name, latency, counts, error strings). See
  `docs/security.md`.

## Keeping docs fresh

Before merging a change that touches any of the following, update the matching
doc file in the **same commit**:

| If you change… | Update… |
|---|---|
| a route or its request/response | `docs/api.md` |
| an env var / deployment shape | `docs/operations.md` + `.env.example` |
| session lifecycle, isolation, or auth | `docs/architecture.md` + new ADR |
| a validation / limit / rejection rule | `docs/security.md` |
| core architectural decision | new file under `docs/adr/` |

## Extending it

- **Tutorials & challenges** — out of scope for MVP. Design sketched in
  `docs/architecture.md` ("Future content pipeline").
- **OAuth / persistent accounts** — would replace the anonymous path in
  `auth.rs`; write a new ADR before doing this.
- **Multi-node ES, Kibana-style features** — out of scope.
