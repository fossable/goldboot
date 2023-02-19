use actix_web::{App, HttpServer};

pub mod api {
    pub mod build;
    pub mod image;
    pub mod media;
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| App::new().service(crate::api::media::download))
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}
