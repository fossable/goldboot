use actix_web::{get, put, web, HttpResponse, Responder, Result};
use goldboot_core::{registry::media::GetMediaResponse, templates::TemplateBase};
use simple_error::bail;
use std::{error::Error, fs::File, path::Path};

///
#[get("/media/{template}/{edition}/{arch}")]
pub async fn download(path: web::Path<(String, String, String)>) -> Result<impl Responder> {
	let (template, edition, arch) = path.into_inner();

	match template.try_into()? {
		TemplateBase::ArchLinux => Ok(web::Json(GetMediaResponse {
			url: String::from(""),
			checksum: None,
		})),
		_ => Err(actix_web::error::ErrorBadRequest("")),
	}
}
