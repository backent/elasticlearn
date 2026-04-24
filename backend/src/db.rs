//! SQLite-backed session store. The only table is `sessions`, which owns the
//! authoritative lifetime for a practice session. If a row disappears, the
//! cleanup task is free to wipe the corresponding Elasticsearch indices.

use chrono::{DateTime, Utc};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Session {
    pub sid: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

pub async fn connect(database_url: &str) -> anyhow::Result<SqlitePool> {
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect(database_url)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

pub async fn insert_session(
    pool: &SqlitePool,
    sid: &str,
    expires_at: DateTime<Utc>,
) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO sessions (sid, expires_at) VALUES (?1, ?2)")
        .bind(sid)
        .bind(expires_at)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_session(pool: &SqlitePool, sid: &str) -> sqlx::Result<Option<Session>> {
    sqlx::query_as::<_, Session>(
        "SELECT sid, created_at, expires_at FROM sessions WHERE sid = ?1",
    )
    .bind(sid)
    .fetch_optional(pool)
    .await
}

pub async fn list_expired(pool: &SqlitePool) -> sqlx::Result<Vec<Session>> {
    sqlx::query_as::<_, Session>(
        "SELECT sid, created_at, expires_at FROM sessions WHERE expires_at < CURRENT_TIMESTAMP",
    )
    .fetch_all(pool)
    .await
}

pub async fn delete_session(pool: &SqlitePool, sid: &str) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM sessions WHERE sid = ?1")
        .bind(sid)
        .execute(pool)
        .await?;
    Ok(())
}
