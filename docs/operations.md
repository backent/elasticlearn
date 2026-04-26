# Operations

## Local development

**Prereqs:** Docker (for Elasticsearch), Rust 1.85+ (some transitive deps now
require `edition2024`), Node.js 20+.

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
| `ADMIN_TOKEN` | *(unset)* | When set, unlocks `GET /api/admin/stats`. Unset/empty → endpoint 404s. |
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

## Analytics

Every interesting interaction is recorded as a row in `events`:

| Column | Example | Notes |
|---|---|---|
| `ts` | `2026-04-24 09:37:12` | server time |
| `sid` | `01htgfx…abc` | session id (opaque ULID) |
| `kind` | `query_executed` | one of: `session_started`, `session_ended`, `dataset_uploaded`, `query_executed` |
| `status` | `ok` / `error` |  |
| `meta` | `{"index":"movies","latency_ms":12,"hits":7}` | JSON blob — **never contains query bodies or uploaded documents** |

Quick endpoint for a glanceable view:

```bash
curl -H "Authorization: Bearer $ADMIN_TOKEN" \
  http://localhost:8080/api/admin/stats | jq
```

Or slice it yourself:

```bash
sqlite3 data.db <<'SQL'
-- sessions started per day, last 14 days
SELECT date(ts), COUNT(*) FROM events
 WHERE kind = 'session_started' AND ts >= datetime('now','-14 days')
 GROUP BY 1 ORDER BY 1;

-- most-hit indices
SELECT json_extract(meta,'$.index') AS idx, COUNT(*) FROM events
 WHERE kind = 'query_executed' GROUP BY 1 ORDER BY 2 DESC LIMIT 10;

-- query latency distribution
SELECT
  COUNT(*) AS n,
  AVG(CAST(json_extract(meta,'$.latency_ms') AS INTEGER)) AS avg_ms,
  MAX(CAST(json_extract(meta,'$.latency_ms') AS INTEGER)) AS max_ms
 FROM events WHERE kind = 'query_executed' AND status = 'ok';

-- top error messages on upload
SELECT json_extract(meta,'$.error') AS err, COUNT(*) FROM events
 WHERE kind = 'dataset_uploaded' AND status = 'error'
 GROUP BY 1 ORDER BY 2 DESC LIMIT 20;
SQL
```

**Retention.** Events are kept indefinitely by default. To cap the table, run
nightly:

```sql
DELETE FROM events WHERE ts < datetime('now','-90 days');
VACUUM;
```

**Privacy.** The event payload intentionally excludes query bodies and
document contents, so dumping the table is safe to share internally. Session
ids are opaque and unlinkable to real users.

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
