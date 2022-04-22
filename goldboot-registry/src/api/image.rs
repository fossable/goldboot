/// Get image info
#[get("/images/{id}")]
pub async fn info(id: web::Path<String>) -> impl Responder {
	todo!();
}

/// Get image list
#[get("/images")]
pub async fn list(id: web::Path<String>) -> impl Responder {
	todo!();
}

/// Push an image
#[put("/images/{id}")]
pub async fn push(id: web::Path<String>, rq: actix_web::HttpRequest) -> impl Responder {
	let path = ImageLibrary::lookup(id);

	// Delete if the image already exists
	if path.exists() {
		std::fs::remove_file(&path)?;
	}

	let mut file = File::create(&path)?;

	std::io::copy(rq, file)?;
	todo!();
}

/// Get cluster data
#[get("/images/{id}/clusters/{range}")]
pub async fn clusters(id: web::Path<String>, range: web::Path<String>) -> impl Responder {
	todo!();
}

/// Get cluster hashes
#[get("/images/{id}/hashes/{range}")]
pub async fn hashes(id: web::Path<String>, range: web::Path<String>) -> impl Responder {
	todo!();
}
