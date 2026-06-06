//! `start` command — serve the HTTP API.
//!
//! The server intentionally does not terminate TLS or perform any
//! authentication. Operators are expected to put nginx (or another reverse
//! proxy) in front of it to handle both. See the project README.

use crate::{api, storage::Storage};
use anyhow::{Context, Result, bail};
use axum::{
    Router,
    routing::{get, put},
};
use clap::Args;
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use tower_http::{
    limit::RequestBodyLimitLayer, set_header::SetResponseHeaderLayer, trace::TraceLayer,
};
use tracing::info;

pub const DEFAULT_MAX_UPLOAD: u64 = 32 * 1024 * 1024 * 1024;

#[derive(Args, Debug)]
pub struct StartArgs {
    /// Address to bind. Should usually be loopback when running behind a
    /// reverse proxy.
    #[clap(long, default_value = "0.0.0.0:3000")]
    pub bind: String,

    /// Directory where uploaded images are stored.
    #[clap(long, default_value = "/var/lib/goldboot-registry")]
    pub data_dir: PathBuf,

    /// Maximum upload size in bytes (default 32 GiB).
    #[clap(long, default_value_t = DEFAULT_MAX_UPLOAD)]
    pub max_upload_size: u64,
}

#[derive(Clone)]
pub struct ServerConfig {
    pub max_upload_size: u64,
}

pub async fn run(args: StartArgs) -> Result<()> {
    let bind: SocketAddr = args
        .bind
        .parse()
        .with_context(|| format!("invalid bind address '{}'", args.bind))?;
    let storage = Arc::new(Storage::new(args.data_dir)?);
    let server_config = ServerConfig {
        max_upload_size: args.max_upload_size,
    };
    let max_upload = args.max_upload_size as usize;

    let app: Router = Router::new()
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
        .layer(axum::Extension(server_config));

    info!(
        addr = %bind,
        "Starting HTTP server"
    );
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    bail!("server exited");
}
