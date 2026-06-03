//! `start` command — load config and serve the HTTP API.

use crate::{
    api,
    auth::{AppState, LoginLimiter, new_token_store, sweep_expired_tokens},
    config::Config,
    storage::Storage,
};
use anyhow::{Context, Result, bail};
use axum::{
    Router,
    routing::{get, post, put},
};
use axum_server::tls_rustls::RustlsConfig;
use std::{net::SocketAddr, path::Path, sync::Arc, time::Duration};
use tower_http::{
    limit::RequestBodyLimitLayer, set_header::SetResponseHeaderLayer, trace::TraceLayer,
};
use tracing::{info, warn};

pub async fn run(config_path: &Path) -> Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();

    let cfg = Config::load(config_path)?;
    let bind: SocketAddr = cfg
        .server
        .bind
        .parse()
        .with_context(|| format!("invalid bind address '{}'", cfg.server.bind))?;
    let storage = Arc::new(Storage::new(&cfg.server.data_dir)?);

    let token_store = new_token_store();
    let limiter = Arc::new(LoginLimiter::new());
    let max_upload = cfg.server.max_upload_size as usize;
    let tls_enabled = cfg.tls_enabled();
    let tls_cert = cfg.server.tls_cert.clone();
    let tls_key = cfg.server.tls_key.clone();

    let state = AppState {
        config: Arc::new(cfg),
        tokens: token_store.clone(),
        limiter,
    };

    // Background sweeper for expired tokens
    {
        let store = token_store.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(60));
            loop {
                tick.tick().await;
                sweep_expired_tokens(&store);
            }
        });
    }

    let app: Router = Router::new()
        .route("/v1/auth/login", post(api::auth::login))
        .route("/v1/auth/logout", post(api::auth::logout))
        .route("/v1/images", get(api::images::list))
        .route(
            "/v1/images/{name}/tags/{tag}/manifest",
            get(api::images::manifest),
        )
        .route(
            "/v1/images/{name}/tags/{tag}/clusters",
            get(api::images::clusters),
        )
        .route("/v1/images/{name}/tags/{tag}", put(api::images::push))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::SERVER,
            axum::http::HeaderValue::from_static("goldboot-registry"),
        ))
        .layer(RequestBodyLimitLayer::new(max_upload))
        .layer(TraceLayer::new_for_http())
        .layer(axum::Extension(storage.clone()))
        .with_state(state);

    if tls_enabled {
        let cert = tls_cert.ok_or_else(|| anyhow::anyhow!("tls_cert required"))?;
        let key = tls_key.ok_or_else(|| anyhow::anyhow!("tls_key required"))?;
        info!(addr = %bind, "starting HTTPS server");
        let tls = RustlsConfig::from_pem_file(Path::new(&cert), Path::new(&key)).await?;
        axum_server::bind_rustls(bind, tls)
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            .await?;
    } else {
        warn!(
            addr = %bind,
            "starting HTTP server (no TLS) — bearer tokens and image data will travel in plaintext"
        );
        let listener = tokio::net::TcpListener::bind(bind).await?;
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await?;
    }

    // Sanity: ensure we never claim "running" without actually bound
    bail!("server exited");
}
