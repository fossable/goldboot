use crate::extract::ImageHandle;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use goldboot::registry::api::image::ImageInfoResponse;

/// Get image info
pub async fn info(image: ImageHandle) -> Json<ImageInfoResponse> {
    Json(image.0.into())
}

/// Get image list
pub async fn list() {}

// Push an image
/*
pub async fn push(id: web::Path<String>, rq: actix_web::HttpRequest) -> Result<HttpResponse> {
    let path = match ImageLibrary::find_by_id(&id) {
        Ok(image) => {
            // Delete if the image already exists
            if Path::new(&image.path).exists() {
                std::fs::remove_file(&image.path)?;
            }
            image.path
        },
        _ => format!("{}.gb", id),
    };

    let mut file = File::create(&path)?;
    std::io::copy(&mut rq, &mut file)?;
    ""
}*/

/// Get cluster data
pub async fn clusters(Path(_id): Path<String>, Path(_range): Path<String>) {}

/// Get cluster hashes
pub async fn hashes(Path(_id): Path<String>, Path(_range): Path<String>) {}
