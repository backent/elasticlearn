# Operations

## Local development

**Prereqs:** Docker (for Elasticsearch), Rust 1.79+, Node.js 20+.

```bash
git clone <repo> && cd elasticlearn
cp .env.example .env                 # edit JWT_SECRET

docker compose up -d elasticsearch   # :9200
( cd backend  && cargo run )         # :8080 — hot restart with cargo-watch
( cd frontend && npm i && npm run dev ) # :5173 with HMR
```

Visit http://localhost:5173.

## Fully containerised

```bash
docker compose up --build
```

The frontend service in `docker-compose.yml` runs Vite in dev mode for
simplicity. For production, replace with a multi-stage build that outputs
static files served by nginx/Caddy.

## Environment variables

| Name | Default | Blast radius if wrong |
|---|---|---|
| `ES_URL` | `http://localhost:9200` | Backend can't reach ES → startup fails with `ES ping status`. |
| `JWT_SECRET` | *required* | Short/predictable → cookies can be forged. Must be ≥ 16 chars. |
| `SESSION_TTL_MIN` | `60` | Too long → indices accumulate. Too short → bad UX. |
| `DATABASE_URL` | `sqlite://data.db?mode=rwc` | Wrong path → migration fails. In Docker, keep it on the `backend-data` volume. |
| `BIND_ADDR` | `0.0.0.0:8080` | Don't bind to a public IP without TLS in front. |
| `MAX_UPLOAD_BYTES` | `52428800` | Larger → OOM risk. Smaller → users hit 413. |
| `MAX_DOCS_PER_INDEX` | `100000` | Governs ES storage per session. |
| `FRONTEND_ORIGIN` | `http://localhost:5173` | If wrong → CORS blocks the SPA. Must match how users load the app. |
| `RUST_LOG` | `info` | Use `debug` while troubleshooting. `elasticlearn=debug` for app-only. |

## Production deployment sketch

Single VM, Docker Compose, Caddy in front for TLS:

```
[internet] ─▶ Caddy (:443) ─▶ frontend(:5173)
                           └▶ backend(:8080)
                                   └▶ elasticsearch(:9200, internal network only)
```

`Caddyfile`:

```
learn.example.com {
  handle /api/* { reverse_proxy backend:8080 }
  handle        { reverse_proxy frontend:5173 }
}
```

Tighten before going public:

- Switch `xpack.security.enabled=true` on ES, generate an API key for the
  backend, pass via `ES_API_KEY` env (not yet wired — add when needed).
- Build the frontend statically (`npm run build`) and serve with Caddy
  directly; remove the dev Vite container.
- Enable `tower-governor` rate limit (TODO in `main.rs`).
- Put `backend-data` on a persistent volume.

## Backup & restore

- **SQLite** (`/data/data.db` in the compose volume) is the only stateful
  component worth backing up. It only contains live session metadata — no
  user data. Daily snapshot is more than enough.
- **Elasticsearch data** is ephemeral by design. Don't back it up; if you
  lose the cluster, live sessions just start fresh.

## Logs and observability

- Tracing emits structured logs via `tracing-subscriber` with `EnvFilter`.
- Cleanup writes `info` when it expires sessions — grep `expiring sessions`
  to see activity.
- `upstream_error` in logs means ES rejected a request; the JSON body is
  included.
- `/api/health` returns plain `ok`. Point your orchestrator's liveness probe
  at it.

## Upgrading Elasticsearch

Pin the tag in `docker-compose.yml`. Because user data is session-scoped and
wiped, major version upgrades don't need a data migration. Bump, run the
compose stack, confirm `/api/health` + `curl :9200` come up.

## Common failures

| Symptom | Likely cause | Fix |
|---|---|---|
| Backend exits immediately with `JWT_SECRET is required` | `.env` not loaded | Ensure `cp .env.example .env` and run from `backend/` |
| `ES ping status 503` on startup | ES not healthy yet | Compose healthcheck usually handles it — retry `docker compose up -d backend` |
| `401 unauthorized` from every endpoint | Cookie not set / TTL elapsed / DB file reset | Click **Start practice** in the UI, or `POST /api/sessions` |
| `403 forbidden` on query | Stale session cookie after DB wipe | Clear cookies, start a new session |
| Indices not wiped after expiry | Cleanup task crashed — check `RUST_LOG=debug` logs | Restart backend; the sweep will catch up on next tick |
