//! Usage analytics. Every interesting interaction is recorded as a row in the
//! `events` table. We deliberately do **not** log query bodies or uploaded
//! documents — that data may be sensitive, and the goal is aggregate usage
//! analysis, not replay.
//!
//! Writes are best-effort: if persistence fails we log and move on, so a
//! broken analytics layer can never take down a user-facing request.

use serde_json::Value;
use sqlx::SqlitePool;

pub const KIND_SESSION_STARTED: &str = "session_started";
pub const KIND_SESSION_ENDED: &str = "session_ended";
pub const KIND_DATASET_UPLOADED: &str = "dataset_uploaded";
pub const KIND_QUERY_EXECUTED: &str = "query_executed";

pub const STATUS_OK: &str = "ok";
pub const STATUS_ERROR: &str = "error";

pub async fn track(
    pool: &SqlitePool,
    sid: Option<&str>,
    kind: &str,
    status: &str,
    meta: Value,
) {
    let meta = serde_json::to_string(&meta).unwrap_or_else(|_| "{}".into());
    let res = sqlx::query(
        "INSERT INTO events (sid, kind, status, meta) VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(sid)
    .bind(kind)
    .bind(status)
    .bind(meta)
    .execute(pool)
    .await;

    if let Err(e) = res {
        tracing::warn!(error = %e, kind, "failed to persist event");
    }
}
