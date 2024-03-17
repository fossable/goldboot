use axum::{routing::get, Router};

pub mod api;
pub mod extract;
pub mod media;

pub struct RegistryTokenPermissions {
    // TODO
}

pub struct RegistryToken {
    /// The token value
    pub token: String,

    /// Whether the token value has been hashed with PBKDF2
    pub hashed: bool,

    /// Whether the token value has been encrypted with AES256
    pub encrypted: bool,

    /// A time-based second factor secret URL associated with the token
    pub totp_secret_url: Option<String>,

    /// The expiration timestamp
    pub expiration: Option<u64>,

    /// The token's associated permissions
    pub permissions: RegistryTokenPermissions,
}

#[derive(Clone)]
pub struct RegistryState {}

pub async fn run(address: String, port: u16) {
    let state = RegistryState {};

    let app = Router::new()
        .route("/image/list", get(api::image::list))
        .route("/image/info/:image_id", get(api::image::info))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("{address}:{port}"))
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}
