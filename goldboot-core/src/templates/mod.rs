use crate::{build::BuildWorker, *};
use serde::{Deserialize, Serialize};
use simple_error::bail;
use std::error::Error;
use validator::Validate;

use crate::templates::{
	alpine::AlpineTemplate, arch_linux::ArchLinuxTemplate, debian::DebianTemplate,
	mac_os::MacOsTemplate, pop_os::PopOsTemplate, steam_deck::SteamDeckTemplate,
	steam_os::SteamOsTemplate, ubuntu_desktop::UbuntuDesktopTemplate,
	ubuntu_server::UbuntuServerTemplate, windows_10::Windows10Template,
};

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

/// Represents a "base configuration" that users can modify and use to build images.
pub trait Template {
	fn build(&self, context: &BuildWorker) -> Result<(), Box<dyn Error>>;

	/// Get the template's multiboot config if applicable.
	fn multiboot_container(&self) -> Option<MultibootContainer> {
		None
	}
}

pub enum TemplateType {
	Alpine,
	ArchLinux,
	Debian,
	MacOs,
	PopOs,
	SteamDeck,
	SteamOs,
	UbuntuDesktop,
	UbuntuServer,
	Windows10,
}

impl TemplateType {
	#[rustfmt::skip]
	pub fn parse_template(value: serde_json::Value) -> Result<Box<dyn Template>, Box<dyn Error>> {
		if let Some(name) = value.get("type") {
			let template: Box<dyn Template> = match name.as_str().unwrap().to_lowercase().as_str() {
				"alpine"        => Box::new(serde_json::from_value::<AlpineTemplate>(value)?),
				"archlinux"     => Box::new(serde_json::from_value::<ArchLinuxTemplate>(value)?),
				"debian"        => Box::new(serde_json::from_value::<DebianTemplate>(value)?),
				//"goldbootusb"   => Box::new(serde_json::from_value::<GoldbootUsbTemplate>(value)?),
				"macos"         => Box::new(serde_json::from_value::<MacOsTemplate>(value)?),
				"popos"         => Box::new(serde_json::from_value::<PopOsTemplate>(value)?),
				"steamdeck"     => Box::new(serde_json::from_value::<SteamDeckTemplate>(value)?),
				"steamos"       => Box::new(serde_json::from_value::<SteamOsTemplate>(value)?),
				"ubuntudesktop" => Box::new(serde_json::from_value::<UbuntuDesktopTemplate>(value)?),
				"ubuntuserver"  => Box::new(serde_json::from_value::<UbuntuServerTemplate>(value)?),
				"windows10"     => Box::new(serde_json::from_value::<Windows10Template>(value)?),
				//"windows11"     => Box::new(serde_json::from_value::<Windows11Template>(value)?),
				//"windows7"      => Box::new(serde_json::from_value::<Windows7Template>(value)?),
				_               => bail!("Unknown template"),
			};

			Ok(template)
		} else {
			bail!("Missing template type");
		}
	}

	#[rustfmt::skip]
	pub fn get_default_template(name: &str) -> Result<serde_json::Value, Box<dyn Error>> {
		Ok(match name.to_lowercase().as_str() {
			"alpine"         => serde_json::to_value(AlpineTemplate::default()),
			"archlinux"      => serde_json::to_value(ArchLinuxTemplate::default()),
			"debian"         => serde_json::to_value(DebianTemplate::default()),
			//"goldbootusb"    => serde_json::to_value(GoldbootUsbTemplate::default()),
			"macos"          => serde_json::to_value(MacOsTemplate::default()),
			"popos"          => serde_json::to_value(PopOsTemplate::default()),
			"steamdeck"      => serde_json::to_value(SteamDeckTemplate::default()),
			"steamos"        => serde_json::to_value(SteamOsTemplate::default()),
			"ubuntudesktop"  => serde_json::to_value(UbuntuDesktopTemplate::default()),
			"ubuntuserver"   => serde_json::to_value(UbuntuServerTemplate::default()),
			"windows10"      => serde_json::to_value(Windows10Template::default()),
			//"windows11"      => serde_json::to_value(Windows11Template::default()),
			//"windows7"       => serde_json::to_value(Windows7Template::default()),
			_                => bail!("Unknown template"),
		}?)
	}
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct IsoContainer {
	/// The installation media URL
	pub iso_url: String,

	/// A hash of the installation media
	pub iso_checksum: String,
}

//static RE_DISK_USAGE: Regex = Regex::new(r"[1-9][0-9]?%$").unwrap();

#[derive(Clone, Serialize, Deserialize, Validate, Debug, Default)]
pub struct MultibootContainer {
	//#[validate(regex = "RE_DISK_USAGE")]
	pub disk_usage: String,
}

impl MultibootContainer {
	pub fn percentage(&self) -> f64 {
		0_f64
	}
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
