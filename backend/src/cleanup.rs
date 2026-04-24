//! Background expiration task. Runs every 30 seconds, finds sessions whose
//! `expires_at` has passed, deletes their Elasticsearch indices, then removes
//! the row from SQLite. Idempotent — running it twice is safe.

use std::time::Duration;

use chrono::Utc;
use serde_json::json;

use crate::{analytics, db, state::AppState};

pub fn spawn(state: AppState) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(30));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            ticker.tick().await;
            if let Err(e) = sweep(&state).await {
                tracing::warn!(error = %e, "cleanup sweep failed");
            }
        }
    });
}

async fn sweep(state: &AppState) -> anyhow::Result<()> {
    let expired = db::list_expired(&state.pool).await?;
    if expired.is_empty() {
        return Ok(());
    }
    tracing::info!(count = expired.len(), "expiring sessions");

    for s in expired {
        let duration_sec = (Utc::now() - s.created_at).num_seconds().max(0);

        if let Err(e) = state.es.delete_session_indices(&s.sid).await {
            tracing::warn!(sid = %s.sid, error = %e, "failed to delete ES indices");
            continue;
        }
        if let Err(e) = db::delete_session(&state.pool, &s.sid).await {
            tracing::warn!(sid = %s.sid, error = %e, "failed to delete session row");
            continue;
        }

        analytics::track(
            &state.pool,
            Some(&s.sid),
            analytics::KIND_SESSION_ENDED,
            analytics::STATUS_OK,
            json!({ "reason": "expired", "duration_sec": duration_sec }),
        )
        .await;
    }
    Ok(())
}
