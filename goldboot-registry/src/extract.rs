use super::RegistryState;
use axum::{
    extract::{FromRequest, Path, Request},
    http::StatusCode,
};
use goldboot::library::ImageLibrary;
use std::collections::HashMap;
use tracing::error;

/// Newtype wrapper for ImageHandle.
pub struct ImageHandle(pub goldboot_image::ImageHandle);

impl FromRequest<RegistryState> for ImageHandle {
    type Rejection = StatusCode;

    async fn from_request(req: Request, state: &RegistryState) -> Result<Self, Self::Rejection> {
        match Path::<HashMap<String, String>>::from_request(req, state).await {
            Ok(value) => match value.get("image_id") {
                Some(image_id) => match ImageLibrary::find_by_id(image_id) {
                    Ok(image_handle) => {
                        if image_handle.primary_header.is_public() {
                            Ok(ImageHandle(image_handle))
                        } else {
                            todo!()
                        }
                    }
                    Err(_) => Err(StatusCode::NOT_FOUND),
                },
                None => {
                    error!("No image id");
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            },
            Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }
}
