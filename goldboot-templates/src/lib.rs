#![feature(derive_default_enum)]

use goldboot_core::*;
use serde::{Deserialize, Serialize};
use simple_error::bail;
use std::error::Error;
use validator::Validate;

pub mod alpine;
pub mod arch_linux;
pub mod debian;
pub mod goldboot_usb;
pub mod mac_os;
pub mod pop_os;
pub mod steam_deck;
pub mod steam_os;
pub mod ubuntu_desktop;
pub mod ubuntu_server;
pub mod windows_10;
pub mod windows_11;
pub mod windows_7;

#[rustfmt::skip]
fn parse_template(value: serde_json::Value) -> Result<Box<dyn Template>, Box<dyn Error>> {
    if let Some(name) = value.get("type") {
        let template: Box<dyn Template> = match name.as_str().unwrap().to_lowercase().as_str() {
            "alpine"        => Box::new(serde_json::from_value::<alpine::AlpineTemplate>(value)?),
            "archlinux"     => Box::new(serde_json::from_value::<arch_linux::ArchLinuxTemplate>(value)?),
            "debian"        => Box::new(serde_json::from_value::<debian::DebianTemplate>(value)?),
            //"goldbootusb"   => Box::new(serde_json::from_value::<goldboot_usb::GoldbootUsbTemplate>(value)?),
            "macos"         => Box::new(serde_json::from_value::<mac_os::MacOsTemplate>(value)?),
            "popos"         => Box::new(serde_json::from_value::<pop_os::PopOsTemplate>(value)?),
            "steamdeck"     => Box::new(serde_json::from_value::<steam_deck::SteamDeckTemplate>(value)?),
            "steamos"       => Box::new(serde_json::from_value::<steam_os::SteamOsTemplate>(value)?),
            "ubuntudesktop" => Box::new(serde_json::from_value::<ubuntu_desktop::UbuntuDesktopTemplate>(value)?),
            "ubuntuserver"  => Box::new(serde_json::from_value::<ubuntu_server::UbuntuServerTemplate>(value)?),
            "windows10"     => Box::new(serde_json::from_value::<windows_10::Windows10Template>(value)?),
            //"windows11"     => Box::new(serde_json::from_value::<windows_11::Windows11Template>(value)?),
            //"windows7"      => Box::new(serde_json::from_value::<windows_7::Windows7Template>(value)?),
            _               => bail!("Unknown template"),
        };

        Ok(template)
    } else {
        bail!("Missing template type");
    }
}

pub fn get_templates(config: &Config) -> Result<Vec<Box<dyn Template>>, Box<dyn Error>> {
    // Precondition: only one of 'template' and 'templates' can exist
    if config.template.is_some() && config.templates.is_some() {
        bail!("'template' and 'templates' cannot be specified simultaneously");
    }

    // Precondition: at least one of 'template' or 'templates' must exist
    if config.template.is_none() && config.templates.is_none() {
        bail!("No template(s) specified");
    }

    let mut templates: Vec<Box<dyn Template>> = Vec::new();

    if let Some(template) = &config.template {
        templates.push(parse_template(template.to_owned())?);
    }

    if let Some(template_array) = &config.templates {
        for template in template_array {
            templates.push(parse_template(template.to_owned())?);
        }
    }

    Ok(templates)
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct IsoContainer {
    /// The installation media URL
    pub iso_url: String,

    /// A hash of the installation media
    pub iso_checksum: String,
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug, Default)]
pub struct MultibootContainer {
    pub disk_usage: String,
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug, Default)]
pub struct ProvisionersContainer {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provisioners: Option<Vec<Provisioner>>,
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug, Default)]
pub struct PartitionsContainer {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partitions: Option<Vec<Partition>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
