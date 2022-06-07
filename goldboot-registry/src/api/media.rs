use actix_web::{get, put, web, HttpResponse, Responder, Result};
use std::{error::Error, fs::File, path::Path};
use simple_error::bail;
use goldboot_core::registry::media::GetMediaResponse;
use goldboot_core::templates::TemplateBase;

///
#[get("/media/{template}/{edition}/{arch}")]
pub async fn download(path: web::Path<(String, String, String)>) -> Result<impl Responder> {
	let (template, edition, arch) = path.into_inner();

	match template.try_into()? {
		TemplateBase::ArchLinux => {
			Ok(web::Json(GetMediaResponse {
				url: String::from(""),
				checksum: None,
			}))
		},
		_ => Err(actix_web::error::ErrorBadRequest("")),
	}
}
