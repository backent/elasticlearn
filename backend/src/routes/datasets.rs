//! Dataset upload. Accepts NDJSON or CSV, streams into Elasticsearch via the
//! `_bulk` endpoint, enforces per-session caps.

use axum::{
    extract::{Multipart, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{auth::AuthSession, error::{AppError, AppResult}, es, state::AppState};

#[derive(Deserialize)]
pub struct UploadParams {
    pub index: String,
    #[serde(default)]
    pub format: Option<String>, // "ndjson" | "csv"
}

#[derive(Serialize)]
pub struct UploadResult {
    pub index: String,
    pub doc_count: usize,
    pub errors: usize,
}

/// POST /api/datasets?index=<name>&format=ndjson|csv — multipart upload.
/// Field: `file` (the dataset). `_id` is optional per doc.
pub async fn upload(
    State(state): State<AppState>,
    AuthSession(s): AuthSession,
    Query(params): Query<UploadParams>,
    mut multipart: Multipart,
) -> AppResult<Json<UploadResult>> {
    let full_index = es::prefixed_index(&s.sid, &params.index)?;
    state.es.create_index(&full_index).await?;

    let format = params
        .format
        .as_deref()
        .unwrap_or("ndjson")
        .to_ascii_lowercase();

    let mut total_bytes = 0usize;
    let mut total_docs = 0usize;
    let mut errors = 0usize;
    let max_docs = state.cfg.max_docs_per_index;
    let max_bytes = state.cfg.max_upload_bytes;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        AppError::BadRequest(format!("multipart error: {e}"))
    })? {
        if field.name() != Some("file") {
            continue;
        }
        let data = field.bytes().await.map_err(|e| {
            AppError::BadRequest(format!("failed to read upload: {e}"))
        })?;
        total_bytes += data.len();
        if total_bytes > max_bytes {
            return Err(AppError::PayloadTooLarge);
        }

        let text = std::str::from_utf8(&data)
            .map_err(|_| AppError::BadRequest("upload must be UTF-8".into()))?;

        let docs: Vec<Value> = match format.as_str() {
            "ndjson" => parse_ndjson(text)?,
            "csv" => parse_csv(text)?,
            other => return Err(AppError::BadRequest(format!("unknown format: {other}"))),
        };

        for chunk in docs.chunks(1000) {
            if total_docs + chunk.len() > max_docs {
                return Err(AppError::PayloadTooLarge);
            }
            let body = build_bulk_body(&full_index, chunk);
            let resp = state.es.bulk_ndjson(body).await?;
            if resp.get("errors").and_then(|v| v.as_bool()).unwrap_or(false) {
                if let Some(items) = resp.get("items").and_then(|v| v.as_array()) {
                    errors += items
                        .iter()
                        .filter(|it| {
                            it.as_object()
                                .and_then(|m| m.values().next())
                                .and_then(|op| op.get("error"))
                                .is_some()
                        })
                        .count();
                }
            }
            total_docs += chunk.len();
        }
    }

    Ok(Json(UploadResult {
        index: params.index,
        doc_count: total_docs,
        errors,
    }))
}

fn parse_ndjson(text: &str) -> AppResult<Vec<Value>> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(line).map_err(|e| {
            AppError::BadRequest(format!("ndjson line {}: {}", i + 1, e))
        })?;
        if !v.is_object() {
            return Err(AppError::BadRequest(format!(
                "ndjson line {}: expected JSON object",
                i + 1
            )));
        }
        out.push(v);
    }
    Ok(out)
}

fn parse_csv(text: &str) -> AppResult<Vec<Value>> {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let header = lines
        .next()
        .ok_or_else(|| AppError::BadRequest("empty CSV".into()))?;
    let headers: Vec<&str> = header.split(',').map(str::trim).collect();

    let mut out = Vec::new();
    for (i, line) in lines.enumerate() {
        let cells: Vec<&str> = line.split(',').map(str::trim).collect();
        if cells.len() != headers.len() {
            return Err(AppError::BadRequest(format!(
                "csv row {}: expected {} columns, got {}",
                i + 2,
                headers.len(),
                cells.len()
            )));
        }
        let mut obj = serde_json::Map::new();
        for (h, c) in headers.iter().zip(cells.iter()) {
            obj.insert((*h).to_string(), guess_scalar(c));
        }
        out.push(Value::Object(obj));
    }
    Ok(out)
}

fn guess_scalar(cell: &str) -> Value {
    if cell.is_empty() {
        return Value::Null;
    }
    if let Ok(n) = cell.parse::<i64>() {
        return Value::from(n);
    }
    if let Ok(f) = cell.parse::<f64>() {
        if let Some(n) = serde_json::Number::from_f64(f) {
            return Value::Number(n);
        }
    }
    match cell {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ => Value::String(cell.to_string()),
    }
}

fn build_bulk_body(full_index: &str, docs: &[Value]) -> String {
    let mut body = String::with_capacity(docs.len() * 128);
    for doc in docs {
        let action = json!({ "index": { "_index": full_index } });
        body.push_str(&action.to_string());
        body.push('\n');
        body.push_str(&doc.to_string());
        body.push('\n');
    }
    body
}
