use axum::extract::{Json, Path, Query};

/// Start a new build
pub async fn start() {
    ""
}

/// List all builds
pub async fn list() {
    ""
}

/// Get build info
pub async fn info(Path(id): Path<String>) {
    ""
}

/// Cancel a build
pub async fn cancel(Path(id): Path<String>) {
    ""
}
