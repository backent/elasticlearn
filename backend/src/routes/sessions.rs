//! Session lifecycle: create, introspect, end.

use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{Duration, Utc};
use serde::Serialize;
use ulid::Ulid;

use crate::{auth, db, error::AppResult, state::AppState};

#[derive(Serialize)]
pub struct SessionView {
    pub session_id: String,
    pub expires_at: chrono::DateTime<Utc>,
    pub ttl_seconds: i64,
}

/// POST /api/sessions — issue a new anonymous session. Replaces any existing
/// cookie; the previous session (if any) will be cleaned up in the background.
pub async fn create(State(state): State<AppState>) -> AppResult<Response> {
    let sid = Ulid::new().to_string().to_ascii_lowercase();
    let expires_at = Utc::now() + Duration::minutes(state.cfg.session_ttl_min);

    db::insert_session(&state.pool, &sid, expires_at).await?;

    let view = SessionView {
        session_id: sid.clone(),
        expires_at,
        ttl_seconds: state.cfg.session_ttl_min * 60,
    };

    let mut resp = (StatusCode::CREATED, Json(view)).into_response();
    resp.headers_mut().insert(
        header::SET_COOKIE,
        auth::issue_cookie(&state.cfg, &sid, expires_at),
    );
    Ok(resp)
}

/// GET /api/sessions/me — return the active session.
pub async fn me(auth::AuthSession(s): auth::AuthSession) -> AppResult<Json<SessionView>> {
    let ttl = (s.expires_at - Utc::now()).num_seconds().max(0);
    Ok(Json(SessionView {
        session_id: s.sid,
        expires_at: s.expires_at,
        ttl_seconds: ttl,
    }))
}

/// DELETE /api/sessions/me — end the current session immediately. The cleanup
/// task will wipe the ES indices on its next tick.
pub async fn end(
    State(state): State<AppState>,
    auth::AuthSession(s): auth::AuthSession,
) -> AppResult<Response> {
    db::delete_session(&state.pool, &s.sid).await?;
    state.es.delete_session_indices(&s.sid).await.ok();

    let mut resp = StatusCode::NO_CONTENT.into_response();
    resp.headers_mut()
        .insert(header::SET_COOKIE, auth::expire_cookie());
    Ok(resp)
}
