pub mod image;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
pub struct RegistryTokenPermissions {
    pub push: bool,
    pub pull: bool,
}

#[derive(Serialize, Deserialize)]
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
