//! Admin-only analytics endpoints. Protected by a constant `ADMIN_TOKEN`
//! presented as `Authorization: Bearer <token>`. When the env var is unset,
//! the endpoint returns 404 — no way to probe for its existence.

use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Serialize;

use crate::{error::AppError, state::AppState};

#[derive(Serialize)]
pub struct Stats {
    pub generated_at: chrono::DateTime<chrono::Utc>,
    pub totals: TotalsByKind,
    pub last_24h: TotalsByKind,
    pub uploads: UploadStats,
    pub queries: QueryStats,
    pub sessions: SessionStats,
    pub events_per_day: Vec<DailyBucket>,
}

#[derive(Serialize, Default)]
pub struct TotalsByKind {
    pub session_started: i64,
    pub session_ended: i64,
    pub dataset_uploaded: i64,
    pub query_executed: i64,
}

#[derive(Serialize, Default)]
pub struct UploadStats {
    pub total: i64,
    pub errors: i64,
    pub total_docs_indexed: i64,
}

#[derive(Serialize, Default)]
pub struct QueryStats {
    pub total: i64,
    pub errors: i64,
    pub avg_latency_ms: Option<f64>,
    pub p95_latency_ms: Option<i64>,
}

#[derive(Serialize, Default)]
pub struct SessionStats {
    pub avg_duration_sec: Option<f64>,
    pub expired: i64,
    pub manual_end: i64,
}

#[derive(Serialize)]
pub struct DailyBucket {
    pub day: String,
    pub session_started: i64,
    pub dataset_uploaded: i64,
    pub query_executed: i64,
}

/// GET /api/admin/stats — aggregate usage analytics.
pub async fn stats(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let token = match state.cfg.admin_token.as_deref() {
        Some(t) => t,
        None => return Err(AppError::NotFound),
    };

    let provided = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .unwrap_or("");

    if provided != token {
        return Err(AppError::Unauthorized);
    }

    let stats = compute_stats(&state.pool).await?;
    Ok((StatusCode::OK, Json(stats)))
}

async fn compute_stats(pool: &sqlx::SqlitePool) -> Result<Stats, AppError> {
    let totals = totals_by_kind(pool, None).await?;
    let last_24h = totals_by_kind(pool, Some("-1 day")).await?;

    let (uploads_ok, uploads_err, docs_indexed): (i64, i64, i64) = sqlx::query_as(
        "SELECT
            COALESCE(SUM(CASE WHEN status = 'ok' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN status = 'error' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CAST(json_extract(meta, '$.doc_count') AS INTEGER)), 0)
         FROM events WHERE kind = 'dataset_uploaded'",
    )
    .fetch_one(pool)
    .await?;

    let (q_ok, q_err, q_avg): (i64, i64, Option<f64>) = sqlx::query_as(
        "SELECT
            COALESCE(SUM(CASE WHEN status = 'ok' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN status = 'error' THEN 1 ELSE 0 END), 0),
            AVG(CAST(json_extract(meta, '$.latency_ms') AS INTEGER))
         FROM events WHERE kind = 'query_executed'",
    )
    .fetch_one(pool)
    .await?;

    let p95 = p95_latency(pool).await?;

    let (avg_dur, expired, manual): (Option<f64>, i64, i64) = sqlx::query_as(
        "SELECT
            AVG(CAST(json_extract(meta, '$.duration_sec') AS INTEGER)),
            COALESCE(SUM(CASE WHEN json_extract(meta, '$.reason') = 'expired' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN json_extract(meta, '$.reason') = 'manual' THEN 1 ELSE 0 END), 0)
         FROM events WHERE kind = 'session_ended'",
    )
    .fetch_one(pool)
    .await?;

    let daily = events_per_day(pool).await?;

    Ok(Stats {
        generated_at: chrono::Utc::now(),
        totals,
        last_24h,
        uploads: UploadStats {
            total: uploads_ok + uploads_err,
            errors: uploads_err,
            total_docs_indexed: docs_indexed,
        },
        queries: QueryStats {
            total: q_ok + q_err,
            errors: q_err,
            avg_latency_ms: q_avg,
            p95_latency_ms: p95,
        },
        sessions: SessionStats {
            avg_duration_sec: avg_dur,
            expired,
            manual_end: manual,
        },
        events_per_day: daily,
    })
}

async fn totals_by_kind(
    pool: &sqlx::SqlitePool,
    since_modifier: Option<&str>,
) -> Result<TotalsByKind, AppError> {
    let sql = match since_modifier {
        Some(_) => {
            "SELECT kind, COUNT(*) FROM events
             WHERE ts >= datetime('now', ?1)
             GROUP BY kind"
        }
        None => "SELECT kind, COUNT(*) FROM events GROUP BY kind",
    };

    let rows: Vec<(String, i64)> = if let Some(modifier) = since_modifier {
        sqlx::query_as(sql).bind(modifier).fetch_all(pool).await?
    } else {
        sqlx::query_as(sql).fetch_all(pool).await?
    };

    let mut t = TotalsByKind::default();
    for (kind, count) in rows {
        match kind.as_str() {
            "session_started" => t.session_started = count,
            "session_ended" => t.session_ended = count,
            "dataset_uploaded" => t.dataset_uploaded = count,
            "query_executed" => t.query_executed = count,
            _ => {}
        }
    }
    Ok(t)
}

async fn p95_latency(pool: &sqlx::SqlitePool) -> Result<Option<i64>, AppError> {
    let latencies: Vec<(i64,)> = sqlx::query_as(
        "SELECT CAST(json_extract(meta, '$.latency_ms') AS INTEGER) AS l
         FROM events
         WHERE kind = 'query_executed' AND status = 'ok' AND l IS NOT NULL
         ORDER BY l ASC",
    )
    .fetch_all(pool)
    .await?;
    if latencies.is_empty() {
        return Ok(None);
    }
    let idx = ((latencies.len() as f64) * 0.95).ceil() as usize - 1;
    Ok(Some(latencies[idx.min(latencies.len() - 1)].0))
}

async fn events_per_day(pool: &sqlx::SqlitePool) -> Result<Vec<DailyBucket>, AppError> {
    let rows: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT date(ts) AS day, kind, COUNT(*)
         FROM events
         WHERE ts >= datetime('now', '-14 days')
         GROUP BY day, kind
         ORDER BY day ASC",
    )
    .fetch_all(pool)
    .await?;

    let mut buckets: std::collections::BTreeMap<String, DailyBucket> =
        std::collections::BTreeMap::new();
    for (day, kind, count) in rows {
        let bucket = buckets.entry(day.clone()).or_insert(DailyBucket {
            day,
            session_started: 0,
            dataset_uploaded: 0,
            query_executed: 0,
        });
        match kind.as_str() {
            "session_started" => bucket.session_started = count,
            "dataset_uploaded" => bucket.dataset_uploaded = count,
            "query_executed" => bucket.query_executed = count,
            _ => {}
        }
    }
    Ok(buckets.into_values().collect())
}

