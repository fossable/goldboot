use anyhow::Result;
use axum::{extract::MatchedPath, http::Request, Router};
use std::{collections::HashMap, path::Path};
use tempfile::TempDir;
use tokio::runtime::Runtime;
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use tracing::{debug, debug_span, info};

/// Minimal HTTP server for serving files to virtual machines
pub struct HttpServer {
    pub port: u16,
    pub address: String,
    pub directory: TempDir,
}

impl HttpServer {
    pub fn new() -> Result<HttpServerBuilder> {
        Ok(HttpServerBuilder {
            router: Router::new(),
            directory: tempfile::tempdir()?,
        })
    }
}

pub struct HttpServerBuilder {
    router: Router,
    directory: TempDir,
}

impl HttpServerBuilder {
    pub fn file<C>(mut self, path: &str, data: C) -> Result<Self>
    where
        C: AsRef<[u8]>,
    {
        let tmp_path = self.directory.path().join(Path::new(path));
        std::fs::create_dir_all(tmp_path.parent().expect("tempdir has a parent"))?;
        std::fs::write(&tmp_path, data)?;

        let path = format!("/{}", path.trim_start_matches("/"));
        debug!(path = %path, tmp_path = ?tmp_path, "Registered HTTP route");
        self.router = self.router.route_service(&path, ServeFile::new(tmp_path));
        Ok(self)
    }

    pub fn serve(self) -> HttpServer {
        let port = crate::find_open_port(8000, 9000);
        info!("Starting HTTP server on port: {}", port);

        let router = self.router.layer(TraceLayer::new_for_http().make_span_with(
            |request: &Request<_>| {
                // Log the matched route's path (with placeholders not filled in).
                // Use request.uri() or OriginalUri if you want the real path.
                let matched_path = request
                    .extensions()
                    .get::<MatchedPath>()
                    .map(MatchedPath::as_str);

                debug_span!(
                    "http_request",
                    method = ?request.method(),
                    matched_path,
                )
            },
        ));

        std::thread::spawn(move || {
            Runtime::new().unwrap().block_on(async move {
                let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
                    .await
                    .unwrap();
                axum::serve(listener, router).await.unwrap();
            });
        });

        HttpServer {
            port,
            address: "10.0.2.2".to_string(),
            directory: self.directory,
        }
    }
}
