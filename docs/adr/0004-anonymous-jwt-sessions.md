# ADR 0004 — Anonymous sessions via signed JWT cookies

**Status:** Accepted (2026-04-24)

## Context

The app is a drop-in playground: users shouldn't have to sign up to try it.
We still need to tie requests to a specific session so the proxy can enforce
per-session index ownership.

## Decision

Each browser gets a signed JWT cookie (`el_session`) on first visit.

- **Cookie attributes:** `HttpOnly`, `SameSite=Strict`, `Path=/`, `Max-Age`
  matches the session TTL.
- **Claims:** `{ sid, exp }` only. No PII, no user identity.
- **Signing:** HMAC-SHA256 with `JWT_SECRET` from env. Minimum 16 chars,
  enforced at startup.
- **Revocation:** `AuthSession` extractor checks the DB row *in addition* to
  the JWT signature. Deleting the row (manual or cleanup task) immediately
  invalidates every future request with that cookie.

## Consequences

- **Positive:** zero-friction start. No email, no OAuth dance.
- **Positive:** no user table, no account management, no password reset flow.
- **Positive:** DB check means we can invalidate a session server-side
  without a blocklist.
- **Negative:** no cross-device continuity. If the user closes the browser
  or switches devices, they lose their data. Acceptable for the learning use
  case (sessions are ephemeral by design).
- **Negative:** rotating `JWT_SECRET` invalidates every live session. This is
  fine in practice; announce a rotation window.
- If we later add accounts, `sid` stays the session identifier and gets paired
  with a `user_id`; existing code keeps working unchanged.
