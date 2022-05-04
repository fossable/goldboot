use crate::{build::BuildWorker, *};
use serde::{Deserialize, Serialize};
use simple_error::bail;
use std::error::Error;
use validator::Validate;

use crate::templates::{
	alpine::AlpineTemplate, arch_linux::ArchLinuxTemplate, debian::DebianTemplate,
	mac_os::MacOsTemplate, pop_os::PopOsTemplate, steam_deck::SteamDeckTemplate,
	steam_os::SteamOsTemplate, ubuntu::UbuntuTemplate, windows_10::Windows10Template,
};

pub mod alpine;
pub mod arch_linux;
pub mod debian;
pub mod goldboot_usb;
pub mod mac_os;
pub mod pop_os;
pub mod steam_deck;
pub mod steam_os;
pub mod ubuntu;
pub mod windows_10;
pub mod windows_11;
pub mod windows_7;

/// Represents a "base configuration" that users can modify and use to build images.
pub trait Template {
	fn build(&self, context: &BuildWorker) -> Result<(), Box<dyn Error>>;

	/// Whether the template can be combined with others.
	fn is_multiboot(&self) -> bool {
		true
	}

	fn general(&self) -> GeneralContainer;
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
#[serde(tag = "type")]
pub enum TemplateType {
	#[default]
	Alpine,
	ArchLinux,
	Debian,
	MacOs,
	PopOs,
	SteamDeck,
	SteamOs,
	Ubuntu,
	Windows10,
}

impl TemplateType {
	#[rustfmt::skip]
	pub fn parse_template(&self, value: serde_json::Value) -> Result<Box<dyn Template>, Box<dyn Error>> {

		Ok(match self {
			TemplateType::Alpine        => Box::new(serde_json::from_value::<AlpineTemplate>(value)?),
			TemplateType::ArchLinux     => Box::new(serde_json::from_value::<ArchLinuxTemplate>(value)?),
			TemplateType::Debian        => Box::new(serde_json::from_value::<DebianTemplate>(value)?),
			//"goldbootusb"   => Box::new(serde_json::from_value::<GoldbootUsbTemplate>(value)?),
			TemplateType::MacOs         => Box::new(serde_json::from_value::<MacOsTemplate>(value)?),
			TemplateType::PopOs         => Box::new(serde_json::from_value::<PopOsTemplate>(value)?),
			TemplateType::SteamDeck     => Box::new(serde_json::from_value::<SteamDeckTemplate>(value)?),
			TemplateType::SteamOs       => Box::new(serde_json::from_value::<SteamOsTemplate>(value)?),
			TemplateType::Ubuntu        => Box::new(serde_json::from_value::<UbuntuTemplate>(value)?),
			TemplateType::Windows10     => Box::new(serde_json::from_value::<Windows10Template>(value)?),
			//"windows11"     => Box::new(serde_json::from_value::<Windows11Template>(value)?),
			//"windows7"      => Box::new(serde_json::from_value::<Windows7Template>(value)?),
		})
	}

	#[rustfmt::skip]
	pub fn get_default_template(&self) -> Result<serde_json::Value, Box<dyn Error>> {
		Ok(match self {
			TemplateType::Alpine         => serde_json::to_value(AlpineTemplate::default()),
			TemplateType::ArchLinux      => serde_json::to_value(ArchLinuxTemplate::default()),
			TemplateType::Debian         => serde_json::to_value(DebianTemplate::default()),
			//"goldbootusb"    => serde_json::to_value(GoldbootUsbTemplate::default()),
			TemplateType::MacOs          => serde_json::to_value(MacOsTemplate::default()),
			TemplateType::PopOs          => serde_json::to_value(PopOsTemplate::default()),
			TemplateType::SteamDeck      => serde_json::to_value(SteamDeckTemplate::default()),
			TemplateType::SteamOs        => serde_json::to_value(SteamOsTemplate::default()),
			TemplateType::Ubuntu         => serde_json::to_value(UbuntuTemplate::default()),
			TemplateType::Windows10      => serde_json::to_value(Windows10Template::default()),
			//"windows11"      => serde_json::to_value(Windows11Template::default()),
			//"windows7"       => serde_json::to_value(Windows7Template::default()),
		}?)
	}
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct IsoContainer {
	/// The installation media URL
	#[serde(rename = "iso_url")]
	pub url: String,

	/// A hash of the installation media
	#[serde(rename = "iso_checksum")]
	pub checksum: String,
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug, Default)]
pub struct ProvisionersContainer {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub provisioners: Option<Vec<Provisioner>>,
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug, Default)]
pub struct GeneralContainer {
	#[serde(flatten)]
	pub r#type: TemplateType,

	#[serde(skip_serializing_if = "Option::is_none")]
	pub qemuargs: Option<Vec<String>>,

	pub storage_size: String,

	#[serde(skip_serializing_if = "Option::is_none")]
	pub partitions: Option<Vec<Partition>>,
}

impl GeneralContainer {
	pub fn storage_size_bytes(&self) -> u64 {
		self.storage_size
			.parse::<ubyte::ByteUnit>()
			.unwrap()
			.as_u64()
	}
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug, Default)]
pub struct RootPasswordContainer {
	// TODO randomize
	pub root_password: String,
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
