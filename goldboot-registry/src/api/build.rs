
/// Start a new build
#[post("/builds")]
pub async fn start() -> impl Responder {
    todo!();
}

/// List all builds
#[get("/builds")]
pub async fn list() -> impl Responder {
    todo!();
}

/// Get build info
#[get("/builds/{id}")]
pub async fn info(id: web::Path<String>) -> impl Responder {
    todo!();
}

/// Cancel a build
#[post("/builds/{id}/cancel")]
pub async fn cancel(id: web::Path<String>) -> impl Responder {
    todo!();
}

