//! Anonymous session auth. A session is represented by a signed JWT stored in
//! an httpOnly cookie. The JWT's only claims are the session id (`sid`) and
//! expiry (`exp`). Validation against the database happens in the extractor,
//! so a cookie whose DB row has been cleaned up is rejected.

use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::{header, request::Parts, HeaderValue},
};
use chrono::{DateTime, Utc};
use cookie::{Cookie, SameSite};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::{config::Config, db, error::AppError, state::AppState};

pub const COOKIE_NAME: &str = "el_session";

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sid: String,
    pub exp: i64,
}

pub fn issue_cookie(cfg: &Config, sid: &str, expires_at: DateTime<Utc>) -> HeaderValue {
    let claims = Claims {
        sid: sid.to_string(),
        exp: expires_at.timestamp(),
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(cfg.jwt_secret.as_bytes()),
    )
    .expect("jwt encode");

    let max_age = (expires_at - Utc::now()).num_seconds().max(0);
    let cookie = Cookie::build((COOKIE_NAME, token))
        .http_only(true)
        .same_site(SameSite::Strict)
        .path("/")
        .max_age(cookie::time::Duration::seconds(max_age))
        .build();

    HeaderValue::from_str(&cookie.to_string()).expect("cookie header value")
}

pub fn expire_cookie() -> HeaderValue {
    let cookie = Cookie::build((COOKIE_NAME, ""))
        .http_only(true)
        .same_site(SameSite::Strict)
        .path("/")
        .max_age(cookie::time::Duration::seconds(0))
        .build();
    HeaderValue::from_str(&cookie.to_string()).expect("cookie header value")
}

/// Axum extractor: the authenticated session, or 401.
pub struct AuthSession(pub db::Session);

#[async_trait]
impl<S> FromRequestParts<S> for AuthSession
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let state = AppState::from_ref(state);

        let token = parts
            .headers
            .get_all(header::COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .flat_map(|h| h.split(';'))
            .map(str::trim)
            .find_map(|pair| {
                let (k, v) = pair.split_once('=')?;
                (k.trim() == COOKIE_NAME).then(|| v.trim().to_string())
            })
            .ok_or(AppError::Unauthorized)?;

        let mut validation = Validation::default();
        validation.leeway = 5;
        let data = decode::<Claims>(
            &token,
            &DecodingKey::from_secret(state.cfg.jwt_secret.as_bytes()),
            &validation,
        )
        .map_err(|_| AppError::Unauthorized)?;

        let session = db::get_session(&state.pool, &data.claims.sid)
            .await?
            .ok_or(AppError::Unauthorized)?;

        if session.expires_at <= Utc::now() {
            return Err(AppError::Unauthorized);
        }

        Ok(AuthSession(session))
    }
}
