# HTTP API

Base path: `/api`. All responses are JSON unless noted. Authentication is via
the `el_session` httpOnly cookie set by `POST /api/sessions`. Endpoints marked
*auth* require that cookie to be valid and the session to be unexpired.

## Common errors

| HTTP | `error` code | Meaning |
|---|---|---|
| 400 | `bad_request` | malformed input (bad JSON, bad index name, script blocked) |
| 401 | `unauthorized` | missing/invalid/expired session cookie |
| 403 | `forbidden` | index name doesn't belong to the caller |
| 404 | `not_found` | index/mapping missing |
| 413 | `payload_too_large` | upload exceeds `MAX_UPLOAD_BYTES` or doc cap |
| 429 | `rate_limited` | per-session rate limit exceeded (future; not live yet) |
| 502 | `upstream_error` | Elasticsearch returned an error |
| 500 | `internal_error` | backend bug — check logs |

Error body:

```json
{ "error": "forbidden", "message": "index not owned by this session" }
```

---

## `POST /api/sessions` — start a session

No body. Creates a new session row, sets `el_session` cookie, returns:

```json
{
  "session_id": "01ht0pvz9z5ygqk9w7t2z3m4x6",
  "expires_at": "2026-04-24T09:15:00Z",
  "ttl_seconds": 3600
}
```

```bash
curl -i -X POST http://localhost:8080/api/sessions
```

## `GET /api/sessions/me` — inspect current session  *(auth)*

Returns the same shape as above, with `ttl_seconds` recomputed from now.

## `DELETE /api/sessions/me` — end session now  *(auth)*

Deletes the DB row, best-effort deletes the user's ES indices, expires the
cookie. Returns 204.

---

## `POST /api/datasets?index=<name>&format=<ndjson|csv>`  *(auth)*

Multipart/form-data. One field required: `file`.

- `index` — 1..=64 chars of `[a-z0-9_-]`, cannot start with `_`. Resolved
  server-side to `session-<sid>-<index>`.
- `format` — defaults to `ndjson`.

Success body:

```json
{ "index": "movies", "doc_count": 10, "errors": 0 }
```

Limits:

- Request body ≤ `MAX_UPLOAD_BYTES` (default 50 MiB).
- Cumulative docs per upload ≤ `MAX_DOCS_PER_INDEX` (default 100 000).
- Each NDJSON line must be a JSON object.

```bash
curl -i -b cookies.txt -F file=@samples/movies.ndjson \
  "http://localhost:8080/api/datasets?index=movies&format=ndjson"
```

---

## `POST /api/query`  *(auth)*

Body:

```json
{
  "index": "movies",
  "body": { "query": { "match_all": {} }, "size": 10 }
}
```

Returns the raw Elasticsearch `_search` response verbatim — this is a
deliberate choice so learners see authentic output.

Rejected bodies:

- Contain `script`, `script_fields`, or `runtime_mappings` anywhere in the
  tree (blocked by `reject_unsafe`).
- Reference an `index` that doesn't begin with the caller's session prefix
  (403 `forbidden`).

```bash
curl -i -b cookies.txt -H "Content-Type: application/json" \
  -d '{"index":"movies","body":{"query":{"match":{"title":"matrix"}}}}' \
  http://localhost:8080/api/query
```

---

## `GET /api/indices`  *(auth)*

Lists indices owned by the caller.

```json
[
  {
    "name": "movies",
    "full_name": "session-01ht…x6-movies",
    "docs": "10",
    "size": "4.1kb"
  }
]
```

## `GET /api/mapping/:name`  *(auth)*

Returns the ES mapping JSON for `session-<sid>-<name>`. 404 if the index
doesn't exist.

---

## `GET /api/health` — liveness probe

Returns plain text `ok`. No auth, no state. Use for container health checks.

---

## `GET /api/admin/stats` — usage analytics  *(admin token)*

Aggregates the `events` table. Requires `Authorization: Bearer <ADMIN_TOKEN>`.
If `ADMIN_TOKEN` is unset, the endpoint returns 404 (not 401) so it can't be
enumerated.

```bash
curl -H "Authorization: Bearer $ADMIN_TOKEN" \
  http://localhost:8080/api/admin/stats
```

Response shape:

```json
{
  "generated_at": "2026-04-24T10:00:00Z",
  "totals": {
    "session_started": 412,
    "session_ended": 389,
    "dataset_uploaded": 201,
    "query_executed": 3104
  },
  "last_24h": { "session_started": 58, "session_ended": 51, "dataset_uploaded": 30, "query_executed": 412 },
  "uploads": { "total": 201, "errors": 4, "total_docs_indexed": 412093 },
  "queries": { "total": 3104, "errors": 18, "avg_latency_ms": 23.4, "p95_latency_ms": 118 },
  "sessions": { "avg_duration_sec": 1840.2, "expired": 321, "manual_end": 68 },
  "events_per_day": [
    { "day": "2026-04-23", "session_started": 57, "dataset_uploaded": 28, "query_executed": 402 }
  ]
}
```

See [`docs/operations.md`](./operations.md#analytics) for raw SQL recipes if
you want to slice the data differently.
