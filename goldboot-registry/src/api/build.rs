use axum::extract::{Path};

/// Start a new build
pub async fn start() {}

/// List all builds
pub async fn list() {}

/// Get build info
pub async fn info(Path(_id): Path<String>) {}

/// Cancel a build
pub async fn cancel(Path(_id): Path<String>) {}
