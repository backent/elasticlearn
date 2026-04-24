//! ElasticLearn backend entry point. Wires config, database, Elasticsearch
//! client, routes, and the cleanup task into a single Axum server.

use std::sync::Arc;

use axum::{
    http::{HeaderValue, Method},
    routing::{delete, get, post},
    Router,
};
use tower_http::{
    cors::CorsLayer, limit::RequestBodyLimitLayer, trace::TraceLayer,
    compression::CompressionLayer,
};

mod auth;
mod cleanup;
mod config;
mod db;
mod error;
mod es;
mod routes;
mod state;

use crate::{config::Config, es::EsClient, state::AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,elasticlearn=debug".into()),
        )
        .init();

    let cfg = Arc::new(Config::from_env()?);
    tracing::info!(es = %cfg.es_url, bind = %cfg.bind_addr, "starting elasticlearn");

    let pool = db::connect(&cfg.database_url).await?;
    let es = EsClient::new(cfg.es_url.clone());
    es.ping().await?;

    let state = AppState {
        cfg: cfg.clone(),
        pool,
        es,
    };

    cleanup::spawn(state.clone());

    let cors = CorsLayer::new()
        .allow_origin(cfg.frontend_origin.parse::<HeaderValue>()?)
        .allow_credentials(true)
        .allow_methods([Method::GET, Method::POST, Method::DELETE])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::ACCEPT,
        ]);

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/sessions", post(routes::sessions::create))
        .route("/api/sessions/me", get(routes::sessions::me))
        .route("/api/sessions/me", delete(routes::sessions::end))
        .route("/api/datasets", post(routes::datasets::upload))
        .route("/api/query", post(routes::query::search))
        .route("/api/indices", get(routes::query::list_indices))
        .route("/api/mapping/:name", get(routes::query::mapping))
        .layer(RequestBodyLimitLayer::new(cfg.max_upload_bytes + 1024))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&cfg.bind_addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        let mut s = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler");
        s.recv().await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received");
}
