//! Thin HTTP client around Elasticsearch. Deliberately *not* a typed client —
//! the whole point of the app is to show raw ES responses to the learner.
//!
//! All public helpers take the user-facing index name (e.g. `"products"`) and
//! a session id, and internally resolve to `session-<sid>-<name>`. Callers
//! must never construct that prefix themselves.

use reqwest::{Client, StatusCode};
use serde_json::Value;

use crate::error::{AppError, AppResult};

#[derive(Clone)]
pub struct EsClient {
    http: Client,
    base: String,
}

/// Allowed characters in a user-chosen index name. Keep it strict so we can
/// safely interpolate into the URL.
fn valid_user_index(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        && !name.starts_with('_')
}

fn valid_sid(sid: &str) -> bool {
    sid.len() == 26 && sid.chars().all(|c| c.is_ascii_alphanumeric())
}

pub fn prefixed_index(sid: &str, user_name: &str) -> AppResult<String> {
    if !valid_sid(sid) {
        return Err(AppError::Forbidden("invalid session".into()));
    }
    if !valid_user_index(user_name) {
        return Err(AppError::BadRequest(
            "index name must be 1-64 chars of [a-z0-9_-] and not start with _".into(),
        ));
    }
    Ok(format!("session-{}-{}", sid.to_ascii_lowercase(), user_name))
}

/// Ensure a fully-qualified index name belongs to the given session.
pub fn assert_owned(sid: &str, full_index: &str) -> AppResult<()> {
    if !valid_sid(sid) {
        return Err(AppError::Forbidden("invalid session".into()));
    }
    let prefix = format!("session-{}-", sid.to_ascii_lowercase());
    if !full_index.starts_with(&prefix) {
        return Err(AppError::Forbidden("index not owned by this session".into()));
    }
    // Defence in depth: disallow `..`, commas, wildcards, slashes.
    if full_index
        .chars()
        .any(|c| c == '*' || c == ',' || c == '/' || c == '?' || c == '"' || c.is_whitespace())
    {
        return Err(AppError::BadRequest("invalid characters in index name".into()));
    }
    Ok(())
}

impl EsClient {
    pub fn new(base: impl Into<String>) -> Self {
        Self {
            http: Client::builder()
                .pool_max_idle_per_host(16)
                .build()
                .expect("reqwest client"),
            base: base.into().trim_end_matches('/').to_string(),
        }
    }

    pub async fn ping(&self) -> AppResult<()> {
        let resp = self.http.get(format!("{}/", self.base)).send().await?;
        if !resp.status().is_success() {
            return Err(AppError::Upstream(format!("ES ping status {}", resp.status())));
        }
        Ok(())
    }

    /// Create an index with a permissive default mapping. Idempotent: a 400
    /// `resource_already_exists_exception` is swallowed so callers can always
    /// "create or reuse".
    pub async fn create_index(&self, full_index: &str) -> AppResult<()> {
        let body = serde_json::json!({
            "settings": { "number_of_shards": 1, "number_of_replicas": 0 }
        });
        let resp = self
            .http
            .put(format!("{}/{}", self.base, full_index))
            .json(&body)
            .send()
            .await?;
        match resp.status() {
            s if s.is_success() => Ok(()),
            StatusCode::BAD_REQUEST => {
                let text = resp.text().await.unwrap_or_default();
                if text.contains("resource_already_exists_exception") {
                    Ok(())
                } else {
                    Err(AppError::Upstream(text))
                }
            }
            s => Err(AppError::Upstream(format!("create_index {}: {}", s, resp.text().await.unwrap_or_default()))),
        }
    }

    /// Send a pre-built NDJSON bulk body to `_bulk`. Caller is responsible
    /// for formatting lines (action line + doc line, trailing newline).
    pub async fn bulk_ndjson(&self, body: String) -> AppResult<Value> {
        let resp = self
            .http
            .post(format!("{}/_bulk", self.base))
            .header("Content-Type", "application/x-ndjson")
            .body(body)
            .send()
            .await?;
        let status = resp.status();
        let json: Value = resp.json().await.unwrap_or(Value::Null);
        if !status.is_success() {
            return Err(AppError::Upstream(format!("_bulk {}: {}", status, json)));
        }
        Ok(json)
    }

    pub async fn search(&self, full_index: &str, body: &Value) -> AppResult<Value> {
        let resp = self
            .http
            .post(format!("{}/{}/_search", self.base, full_index))
            .json(body)
            .send()
            .await?;
        let status = resp.status();
        let json: Value = resp.json().await.unwrap_or(Value::Null);
        if !status.is_success() {
            return Err(AppError::Upstream(format!("_search {}: {}", status, json)));
        }
        Ok(json)
    }

    pub async fn get_mapping(&self, full_index: &str) -> AppResult<Value> {
        let resp = self
            .http
            .get(format!("{}/{}/_mapping", self.base, full_index))
            .send()
            .await?;
        let status = resp.status();
        let json: Value = resp.json().await.unwrap_or(Value::Null);
        if status == StatusCode::NOT_FOUND {
            return Err(AppError::NotFound);
        }
        if !status.is_success() {
            return Err(AppError::Upstream(format!("_mapping {}: {}", status, json)));
        }
        Ok(json)
    }

    /// List indices matching `session-<sid>-*` via `_cat/indices?format=json`.
    pub async fn list_session_indices(&self, sid: &str) -> AppResult<Value> {
        if !valid_sid(sid) {
            return Err(AppError::Forbidden("invalid session".into()));
        }
        let pattern = format!("session-{}-*", sid.to_ascii_lowercase());
        let resp = self
            .http
            .get(format!(
                "{}/_cat/indices/{}?format=json&h=index,docs.count,store.size",
                self.base, pattern
            ))
            .send()
            .await?;
        let status = resp.status();
        let json: Value = resp.json().await.unwrap_or(Value::Array(vec![]));
        if !status.is_success() {
            return Err(AppError::Upstream(format!("_cat/indices {}", status)));
        }
        Ok(json)
    }

    /// Delete every index owned by a session. Safe to call after the indices
    /// are already gone — a 404 is treated as success.
    pub async fn delete_session_indices(&self, sid: &str) -> AppResult<()> {
        if !valid_sid(sid) {
            return Err(AppError::Forbidden("invalid session".into()));
        }
        let pattern = format!("session-{}-*", sid.to_ascii_lowercase());
        let resp = self
            .http
            .delete(format!(
                "{}/{}?ignore_unavailable=true&allow_no_indices=true",
                self.base, pattern
            ))
            .send()
            .await?;
        let status = resp.status();
        if status.is_success() || status == StatusCode::NOT_FOUND {
            return Ok(());
        }
        Err(AppError::Upstream(format!(
            "delete {}: {}",
            status,
            resp.text().await.unwrap_or_default()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_wildcards_in_owned_check() {
        let sid = "01HT0PVZ9Z5YGQK9W7T2Z3M4X6";
        assert!(assert_owned(sid, &format!("session-{}-*", sid.to_ascii_lowercase())).is_err());
        assert!(assert_owned(sid, &format!("session-{}-foo", sid.to_ascii_lowercase())).is_ok());
    }

    #[test]
    fn rejects_cross_session() {
        let sid_a = "01HT0PVZ9Z5YGQK9W7T2Z3M4X6";
        let sid_b = "01HT0PVZ9Z5YGQK9W7T2Z3M4XZ";
        let foreign = format!("session-{}-foo", sid_b.to_ascii_lowercase());
        assert!(assert_owned(sid_a, &foreign).is_err());
    }
}
