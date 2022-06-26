use actix_web::{get, put, web, HttpResponse, Responder};
use goldboot::library::ImageLibrary;
use std::{error::Error, fs::File, path::Path};

/// Get image info
#[get("/images/{id}")]
pub async fn info(id: web::Path<String>) -> impl Responder {
	""
}

/// Get image list
#[get("/images")]
pub async fn list(id: web::Path<String>) -> impl Responder {
	""
}

/// Push an image
/*#[put("/images/{id}")]
pub async fn push(id: web::Path<String>, rq: actix_web::HttpRequest) -> Result<HttpResponse, Box<dyn Error>> {
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
#[get("/images/{id}/clusters/{range}")]
pub async fn clusters(id: web::Path<String>, range: web::Path<String>) -> impl Responder {
	""
}

/// Get cluster hashes
#[get("/images/{id}/hashes/{range}")]
pub async fn hashes(id: web::Path<String>, range: web::Path<String>) -> impl Responder {
	""
}
