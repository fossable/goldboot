use actix_web::{get, post, web, Responder};

/// Start a new build
#[post("/builds")]
pub async fn start() -> impl Responder {
	""
}

/// List all builds
#[get("/builds")]
pub async fn list() -> impl Responder {
	""
}

/// Get build info
#[get("/builds/{id}")]
pub async fn info(id: web::Path<String>) -> impl Responder {
	""
}

/// Cancel a build
#[post("/builds/{id}/cancel")]
pub async fn cancel(id: web::Path<String>) -> impl Responder {
	""
}
