# Security model

The app runs arbitrary Elasticsearch queries against user-uploaded data. Many
of the decisions below trade capability for blast-radius containment.

## Threat model

| # | Threat | Mitigation | Enforced in |
|---|---|---|---|
| 1 | Session A reads or deletes Session B's indices | Every index is prefixed `session-<sid>-`. Every handler resolves names through `prefixed_index`; `assert_owned` rejects anything else. | `backend/src/es.rs`, `routes/query.rs` |
| 2 | Script injection via Painless | Any body containing `script`, `script_fields`, or `runtime_mappings` is rejected with 400. Scripts are off by default. | `routes/query.rs::reject_unsafe` |
| 3 | Oversized upload exhausts memory/disk | Multipart body limit via `RequestBodyLimitLayer(MAX_UPLOAD_BYTES)`. Per-index doc cap checked while streaming. Bulk chunked at 1 000 docs. | `main.rs`, `routes/datasets.rs` |
| 4 | Admin endpoint abuse (`_cluster/*`, `_reindex`, `_nodes/*`) | The proxy only ever constructs URLs for `_search`, `_count`, `_mapping`, `_bulk`, and `_cat/indices/<pattern>`. There is no passthrough route. | `backend/src/es.rs` |
| 5 | Session cookie theft via XSS | Cookies are `HttpOnly`, `SameSite=Strict`, path `/`. The SPA doesn't read them. | `auth::issue_cookie` |
| 6 | CSRF | `SameSite=Strict` cookie + CORS allow-list restricted to `FRONTEND_ORIGIN`. Browsers won't attach the cookie to cross-site POSTs. | `auth::issue_cookie`, `main.rs` CORS layer |
| 7 | Indices outlive the session | 30-second cleanup task deletes `session-<sid>-*` for every expired DB row. Task is idempotent. | `cleanup.rs` |
| 8 | Forged JWT | HMAC-SHA256 signed with `JWT_SECRET` (≥ 16 chars). Expired/invalid tokens return 401. DB row is also checked — revocation works by deleting the row. | `auth.rs`, `config.rs` |
| 9 | Path-traversal / wildcard via index name | `prefixed_index` limits input to `[a-z0-9_-]{1,64}` and rejects leading `_`. `assert_owned` separately bans `*,/?"`/whitespace in the full name. | `backend/src/es.rs` |
| 10 | Denial of service via rapid queries | Rate limit planned via `tower-governor` (scope: per-session). Not live in MVP — document and ship before public launch. | `TODO` `main.rs` |
| 11 | CSV injection into spreadsheets (if someone re-exports) | Out of scope — we never export, and all output is rendered as JSON. | n/a |
| 12 | Admin stats endpoint enumeration | `/api/admin/stats` returns 404 (not 401) when `ADMIN_TOKEN` is unset; when set, a wrong token returns 401 with no token hint in the response. | `routes/admin.rs::stats` |
| 13 | Analytics leaks user data | `analytics::track` only stores session id + kind + status + structural meta (index name, latency, doc count, error string). Query bodies and document contents are never passed in. | `backend/src/analytics.rs`, `routes/*.rs` call sites |

## Explicitly not protected against

- **Hostile cluster operator.** Anyone with shell on the Elasticsearch node
  can read every session's data. Acceptable for a learning app; not
  acceptable for PII.
- **User uploads copyrighted / illegal content.** No scanning. Document in
  ToS.
- **Traffic analysis.** TLS is the operator's responsibility (run behind
  Caddy/nginx in prod).
- **Long-lived data confidentiality after expiry.** ES delete is logical; the
  segments may live on disk until merge. Not a concern in a wiped container.

## Secrets

- `JWT_SECRET` — 32 random bytes minimum. Rotate by deploying a new secret;
  every live session is invalidated on rotation (acceptable).
- No database password in MVP (SQLite file ACL only).
- `ES_URL` assumed to be an unauthenticated dev cluster. For production move
  to HTTPS + basic auth + ES API key, stored in an env secret.

## Pen-test checklist

Before public launch, manually verify:

1. `curl /api/query` with no cookie → 401.
2. `POST /api/query {"index":"session-OTHER-foo",…}` → 403.
3. `POST /api/query` with `{"script":…}` anywhere in body → 400.
4. Upload 60 MiB file → 413.
5. Upload an NDJSON with 200 000 lines → 413 mid-stream.
6. Delete the session row by hand, wait 30 s → ES indices gone (`GET /_cat/indices`).
7. Access the frontend from a different origin → CORS blocks it.
