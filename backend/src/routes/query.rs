//! Query proxy. Every request carries a user-visible index name; the server
//! resolves it to `session-<sid>-<name>` before forwarding to Elasticsearch.

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use std::time::Instant;

use crate::{analytics, auth::AuthSession, error::{AppError, AppResult}, es, state::AppState};

#[derive(Deserialize)]
pub struct SearchRequest {
    pub index: String,
    pub body: Value,
}

#[derive(Serialize)]
pub struct IndexInfo {
    pub name: String,
    pub full_name: String,
    pub docs: Option<String>,
    pub size: Option<String>,
}

/// POST /api/query — run a `_search` scoped to the caller's session.
pub async fn search(
    State(state): State<AppState>,
    AuthSession(s): AuthSession,
    Json(req): Json<SearchRequest>,
) -> AppResult<Json<Value>> {
    let started = Instant::now();
    let outcome: AppResult<Value> = async {
        reject_unsafe(&req.body)?;
        let full = es::prefixed_index(&s.sid, &req.index)?;
        es::assert_owned(&s.sid, &full)?;
        state.es.search(&full, &req.body).await
    }
    .await;

    let latency_ms = started.elapsed().as_millis() as u64;
    let (status, meta) = match &outcome {
        Ok(body) => {
            let hits = body
                .get("hits")
                .and_then(|h| h.get("total"))
                .and_then(|t| t.get("value"))
                .and_then(|v| v.as_u64());
            (
                analytics::STATUS_OK,
                serde_json::json!({
                    "index": req.index,
                    "latency_ms": latency_ms,
                    "hits": hits,
                }),
            )
        }
        Err(e) => (
            analytics::STATUS_ERROR,
            serde_json::json!({
                "index": req.index,
                "latency_ms": latency_ms,
                "error": e.to_string(),
            }),
        ),
    };
    analytics::track(
        &state.pool,
        Some(&s.sid),
        analytics::KIND_QUERY_EXECUTED,
        status,
        meta,
    )
    .await;

    outcome.map(Json)
}

/// GET /api/indices — list the caller's indices.
pub async fn list_indices(
    State(state): State<AppState>,
    AuthSession(s): AuthSession,
) -> AppResult<Json<Vec<IndexInfo>>> {
    let raw = state.es.list_session_indices(&s.sid).await?;
    let prefix = format!("session-{}-", s.sid);
    let items: Vec<IndexInfo> = raw
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|row| {
                    let full = row.get("index")?.as_str()?.to_string();
                    let name = full.strip_prefix(&prefix)?.to_string();
                    Some(IndexInfo {
                        name,
                        full_name: full,
                        docs: row
                            .get("docs.count")
                            .and_then(|v| v.as_str().map(|s| s.to_string())),
                        size: row
                            .get("store.size")
                            .and_then(|v| v.as_str().map(|s| s.to_string())),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Json(items))
}

/// GET /api/mapping/:name — fetch the mapping for one of the caller's indices.
pub async fn mapping(
    State(state): State<AppState>,
    AuthSession(s): AuthSession,
    Path(name): Path<String>,
) -> AppResult<Json<Value>> {
    let full = es::prefixed_index(&s.sid, &name)?;
    es::assert_owned(&s.sid, &full)?;
    let body = state.es.get_mapping(&full).await?;
    Ok(Json(body))
}

/// Block known-dangerous fields. Errs on the side of rejecting — learners who
/// need scripts can unlock them later via a feature flag.
fn reject_unsafe(body: &Value) -> AppResult<()> {
    fn walk(v: &Value) -> bool {
        match v {
            Value::Object(map) => map.iter().any(|(k, v)| {
                if k == "script" || k == "script_fields" || k == "runtime_mappings" {
                    return true;
                }
                walk(v)
            }),
            Value::Array(arr) => arr.iter().any(walk),
            _ => false,
        }
    }
    if walk(body) {
        return Err(AppError::BadRequest(
            "scripted queries are disabled in this playground".into(),
        ));
    }
    Ok(())
}
