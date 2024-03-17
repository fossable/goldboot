use std::collections::HashMap;

use axum::{
    async_trait,
    extract::{rejection::JsonRejection, FromRequest, MatchedPath, Path, Request},
    http::StatusCode,
    response::IntoResponse,
    RequestPartsExt,
};
use tracing::error;

use crate::library::ImageLibrary;

use super::RegistryState;

/// Newtype wrapper for ImageHandle.
pub struct ImageHandle(pub goldboot_image::ImageHandle);

#[async_trait]
impl FromRequest<RegistryState> for ImageHandle {
    type Rejection = StatusCode;

    async fn from_request(req: Request, state: &RegistryState) -> Result<Self, Self::Rejection> {
        let (mut parts, body) = req.into_parts();

        // We can use other extractors to provide better rejection messages.
        // For example, here we are using `axum::extract::MatchedPath` to
        // provide a better error message.
        //
        // Have to run that first since `Json` extraction consumes the request.
        let path = parts
            .extract::<MatchedPath>()
            .await
            .map(|path| path.as_str().to_owned())
            .ok();

        let req = Request::from_parts(parts, body);

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
