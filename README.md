# ElasticLearn

A sandboxed web playground where developers learn Elasticsearch by uploading
their own dataset and running queries against it. Every session is
time-boxed; when it ends, all of the user's indices are wiped.

```
┌─ Workspace ──────┬─ Query ─────────────────┬─ Response ──────────────┐
│ Upload NDJSON    │ {                       │ {                       │
│ Upload CSV       │   "query": {            │   "took": 3,            │
│                  │     "match": {          │   "hits": {             │
│ movies  (10 docs)│       "title": "matrix" │     "total": { … },     │
│ products         │     }                   │     "hits": [ … ]       │
│                  │   }                     │   }                     │
│                  │ }                       │ }                       │
└──────────────────┴─────────────────────────┴─────────────────────────┘
                                                           ⏱ 58:42
```

## Quickstart

**With Docker Compose (recommended):**

```bash
cp .env.example .env     # edit JWT_SECRET!
docker compose up --build
open http://localhost:5173
```

**Local dev (hot reload):**

```bash
# Terminal 1 — Elasticsearch
docker compose up -d elasticsearch

# Terminal 2 — backend
cp .env.example .env
cd backend && cargo run

# Terminal 3 — frontend
cd frontend && npm install && npm run dev
```

## What a session looks like

1. Open the app → backend drops an httpOnly cookie with a signed session token.
   Timer in the header shows how long until wipe (default 60 minutes).
2. Upload a dataset (NDJSON or CSV) — it's indexed as
   `session-<your-id>-<your-chosen-name>`.
3. Write Elasticsearch queries in the Monaco editor, hit **Run**, see the raw
   response.
4. When the timer hits zero — or you click **End & restart** — the cleanup
   task deletes every index you created.

## Project layout

```
backend/    Rust + Axum API (auth, proxy, cleanup)
frontend/   React + Vite + Monaco editor
docs/       Architecture, API, security, operations, ADRs
samples/    Example datasets you can upload
```

Deep docs live in [`docs/`](./docs/). New contributors (including AI) should
start at [`CLAUDE.md`](./CLAUDE.md).

## Environment variables

See [`.env.example`](./.env.example) for the full list and
[`docs/operations.md`](./docs/operations.md) for what each one does.

## License

MIT.
