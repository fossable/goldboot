use goldboot_image::ImageArch;
use serde::{Deserialize, Serialize};
use simple_error::bail;
use std::error::Error;

#[derive(Serialize, Deserialize)]
pub struct GetMediaResponse {
    pub url: String,
    pub checksum: Option<String>,
}

pub fn get_media(
    template: String,
    edition: String,
    arch: ImageArch,
) -> Result<GetMediaResponse, Box<dyn Error>> {
    let rs = reqwest::blocking::get(format!(
        "https://public.goldboot.org/v1/media/{template}/{edition}/{}",
        arch.to_string()
    ))?;
    if rs.status().is_success() {
        let rs = rs.json::<GetMediaResponse>()?;
        return Ok(rs);
    } else {
        bail!("Request failed");
    }
}
