use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename = "unattend")]
pub struct UnattendXml {
	pub xmlns: String,
	pub settings: Vec<Settings>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename = "settings")]
pub struct Settings {
	pub pass: String,
	pub component: Vec<Component>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename = "component")]
pub struct Component {
	pub name: String,
	pub processorArchitecture: String,
	pub publicKeyToken: String,
	pub language: String,
	pub versionScope: String,
	pub ComputerName: Option<ComputerName>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ComputerName {
	#[serde(rename = "$value")]
	pub value: String,
}