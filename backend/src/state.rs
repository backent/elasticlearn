//! Shared application state cloned into every handler.

use std::sync::Arc;

use crate::{config::Config, es::EsClient};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub cfg: Arc<Config>,
    pub pool: SqlitePool,
    pub es: EsClient,
}
