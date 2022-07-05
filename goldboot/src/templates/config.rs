use crate::{
	build::BuildConfig,
	provisioners::{AnsibleProvisioner, ScriptProvisioner, ShellProvisioner},
	ssh::SshConnection,
	Promptable,
};
use serde::{Deserialize, Serialize};
use std::{error::Error, path::Path};
use validator::Validate;

///
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct IsoContainer {
	/// The installation media URL
	pub url: String,

	/// A hash of the installation media
	pub checksum: String,
}

impl Promptable for IsoContainer {
	fn prompt(
		&mut self,
		config: &BuildConfig,
		theme: &dialoguer::theme::ColorfulTheme,
	) -> Result<(), Box<dyn Error>> {
		self.url = dialoguer::Input::with_theme(theme)
			.with_prompt("Enter the ISO URL")
			.interact()?;

		// TODO validate URL

		Ok(())
	}
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug, Default)]
pub struct ProvisionersContainer {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub provisioners: Option<Vec<serde_json::Value>>,
}

impl ProvisionersContainer {
	pub fn run(&self, ssh: &mut SshConnection) -> Result<(), Box<dyn Error>> {
		if let Some(provisioners) = &self.provisioners {
			for provisioner in provisioners {
				match provisioner.get("type").unwrap().as_str().unwrap() {
					"ansible" => {
						let provisioner: AnsibleProvisioner =
							serde_json::from_value(provisioner.to_owned())?;
						provisioner.run(ssh)?;
					}
					"shell" => {
						let provisioner: ShellProvisioner =
							serde_json::from_value(provisioner.to_owned())?;
						provisioner.run(ssh)?;
					}
					"script" => {
						let provisioner: ScriptProvisioner =
							serde_json::from_value(provisioner.to_owned())?;
						provisioner.run(ssh)?;
					}
					_ => {}
				}
			}
		}
		Ok(())
	}
}

impl Promptable for ProvisionersContainer {
	fn prompt(
		&mut self,
		config: &BuildConfig,
		theme: &dialoguer::theme::ColorfulTheme,
	) -> Result<(), Box<dyn Error>> {
		loop {
			if !dialoguer::Confirm::with_theme(theme)
				.with_prompt("Do you want to add a provisioner?")
				.interact()?
			{
				break;
			}

			// Prompt provisioner type
			{
				let provisioner_index = dialoguer::Select::with_theme(theme)
					.with_prompt("Choose provisioner type")
					.item("Shell script")
					.item("Ansible playbook")
					.interact()?;
			}
		}
		todo!()
	}
}
