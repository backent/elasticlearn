//! Background expiration task. Runs every 30 seconds, finds sessions whose
//! `expires_at` has passed, deletes their Elasticsearch indices, then removes
//! the row from SQLite. Idempotent — running it twice is safe.

use std::time::Duration;

use crate::{db, state::AppState};

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
        if let Err(e) = state.es.delete_session_indices(&s.sid).await {
            tracing::warn!(sid = %s.sid, error = %e, "failed to delete ES indices");
            continue;
        }
        if let Err(e) = db::delete_session(&state.pool, &s.sid).await {
            tracing::warn!(sid = %s.sid, error = %e, "failed to delete session row");
        }
    }
    Ok(())
}
