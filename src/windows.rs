use serde::Serialize;
use std::{
    path::Path,
    error::Error,
};

#[derive(Clone, Serialize)]
#[serde(rename = "unattend")]
pub struct UnattendXml {
    pub xmlns: String,
    pub settings: Vec<Settings>,
}

impl UnattendXml {
    pub fn write(&self, path: &Path) -> Result<(), Box<dyn Error>> {
        std::fs::write(
            path.join("Autounattend.xml"),
            quick_xml::se::to_string(&self).unwrap(),
        )
        .unwrap();
        Ok(())
    }
}

#[derive(Clone, Serialize)]
#[serde(rename = "settings")]
pub struct Settings {
    pub pass: String,
    pub component: Vec<Component>,
}

#[derive(Clone, Serialize)]
#[serde(rename = "component")]
pub struct Component {
    pub name: String,
    pub processorArchitecture: String,
    pub publicKeyToken: String,
    pub language: String,
    pub versionScope: String,
    pub ComputerName: Option<ComputerName>,
    pub DiskConfiguration: Option<DiskConfiguration>,
    pub ImageInstall: Option<ImageInstall>,
}

#[derive(Clone, Serialize)]
pub struct DiskConfiguration {
    pub WillShowUI: WillShowUI,
    pub Disk: Disk,
}

#[derive(Clone, Serialize)]
pub struct Disk {
    pub CreatePartitions: CreatePartitions,
    pub ModifyPartitions: ModifyPartitions,
    pub WillWipeDisk: WillWipeDisk,
    pub DiskID: DiskID,
}

#[derive(Clone, Serialize)]
pub struct CreatePartitions {
    pub CreatePartition: Vec<CreatePartition>,
}

#[derive(Clone, Serialize)]
pub struct CreatePartition {
    pub Order: Order,
    pub Size: Size,
    pub Type: Type,
}

#[derive(Clone, Serialize)]
pub struct ModifyPartitions {
    pub ModifyPartition: Vec<ModifyPartition>,
}

#[derive(Clone, Serialize)]
pub struct ModifyPartition {
    pub Format: Format,
    pub Label: Label,
    pub Order: Order,
    pub PartitionID: PartitionID,
    pub Extend: Option<Extend>,
    pub Letter: Option<Letter>,
}

#[derive(Clone, Serialize)]
pub struct ImageInstall {
    pub OSImage: OSImage,
}

#[derive(Clone, Serialize)]
pub struct OSImage {
    pub InstallTo: InstallTo,
    pub WillShowUI: WillShowUI,
    pub InstallToAvailablePartition: InstallToAvailablePartition,
}

#[derive(Clone, Serialize)]
pub struct InstallTo {
    pub DiskID: DiskID,
    pub PartitionID: PartitionID,
}

#[derive(Clone, Serialize)]
pub struct InstallToAvailablePartition {
    #[serde(rename = "$value")]
    pub value: String,
}

#[derive(Clone, Serialize)]
pub struct Format {
    #[serde(rename = "$value")]
    pub value: String,
}

#[derive(Clone, Serialize)]
pub struct Label {
    #[serde(rename = "$value")]
    pub value: String,
}

#[derive(Clone, Serialize)]
pub struct PartitionID {
    #[serde(rename = "$value")]
    pub value: String,
}

#[derive(Clone, Serialize)]
pub struct Letter {
    #[serde(rename = "$value")]
    pub value: String,
}

#[derive(Clone, Serialize)]
pub struct Extend {
    #[serde(rename = "$value")]
    pub value: String,
}

#[derive(Clone, Serialize)]
pub struct DiskID {
    #[serde(rename = "$value")]
    pub value: String,
}

#[derive(Clone, Serialize)]
pub struct WillWipeDisk {
    #[serde(rename = "$value")]
    pub value: String,
}

#[derive(Clone, Serialize)]
pub struct Order {
    #[serde(rename = "$value")]
    pub value: String,
}

#[derive(Clone, Serialize)]
pub struct Size {
    #[serde(rename = "$value")]
    pub value: String,
}

#[derive(Clone, Serialize)]
pub struct Type {
    #[serde(rename = "$value")]
    pub value: String,
}

#[derive(Clone, Serialize)]
pub struct WillShowUI {
    #[serde(rename = "$value")]
    pub value: String,
}

#[derive(Clone, Serialize)]
pub struct ComputerName {
    #[serde(rename = "$value")]
    pub value: String,
}
