use crate::{
	build::BuildWorker,
	cache::{MediaCache, MediaFormat},
	provisioners::*,
	qemu::QemuArgs,
	templates::*,
};
use serde::{Deserialize, Serialize};
use std::error::Error;
use validator::Validate;

use super::*;

#[derive(rust_embed::RustEmbed)]
#[folder = "res/Windows10/"]
struct Resources;

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct Windows10Template {
	pub id: TemplateId,

	username: String,

	password: String,

	hostname: String,

	pub iso: IsoProvisioner,
	pub ansible: Option<Vec<AnsibleProvisioner>>,
}

impl Default for Windows10Template {
	fn default() -> Self {
		Self {
			id: TemplateId::Windows10,
			username: String::from("admin"),
			password: String::from("admin"),
			hostname: String::from("goldboot"),
			iso: IsoContainer {
				url: String::from("<ISO URL>"),
				checksum: String::from("<ISO HASH>"),
			},
			general: GeneralContainer {
				base: TemplateBase::Windows10,
				storage_size: String::from("40 GiB"),
				..Default::default()
			},
			provisioners: ProvisionersContainer::default(),
		}
	}
}

impl Windows10Template {
	fn create_unattended(&self) -> UnattendXml {
		UnattendXml {
			xmlns: "urn:schemas-microsoft-com:unattend".into(),
			settings: vec![Settings {
				pass: "specialize",
				component: vec![Component {
					name: "Microsoft-Windows-Shell-Setup".into(),
					processorArchitecture: "amd64".into(),
					publicKeyToken: "31bf3856ad364e35".into(),
					language: "neutral".into(),
					versionScope: "nonSxS".into(),
					ComputerName: Some(ComputerName {
						value: self.hostname.clone(),
					}),
					DiskConfiguration: None,
					ImageInstall: None,
				}],
			}],
		}
	}
}

impl Template for Windows10Template {
	fn build(&self, context: &BuildWorker) -> Result<(), Box<dyn Error>> {
		let mut qemuargs = QemuArgs::new(&context);

		qemuargs.drive.push(format!(
			"file={},if=ide,cache=writeback,discard=ignore,format=qcow2",
			context.image_path
		));
		qemuargs.drive.push(format!(
			"file={},media=cdrom",
			MediaCache::get(self.iso.url.clone(), &self.iso.checksum, MediaFormat::Iso)?
		));

		// Write the Autounattend.xml file
		//self.create_unattended().write(&context)?;

		// Copy powershell scripts
		//if let Some(resource) = Resources::get("configure_winrm.ps1") {
		//    std::fs::write(context.join("configure_winrm.ps1"), resource.data)?;
		//}

		// Start VM
		let mut qemu = qemuargs.start_process()?;

		// Send boot command
		#[rustfmt::skip]
		qemu.vnc.boot_command(vec![
			wait!(4),
			enter!(),
		])?;

		// Wait for SSH
		let mut ssh = qemu.ssh_wait(context.ssh_port, &self.username, &self.password)?;

		// Run provisioners
		self.provisioners.run(&mut ssh)?;

		// Shutdown
		ssh.shutdown("shutdown /s /t 0 /f /d p:4:1")?;
		qemu.shutdown_wait()?;
		Ok(())
	}
}

impl PromptMut for Windows10Template {
	fn prompt(
		&mut self,
		config: &BuildConfig,
		theme: &dialoguer::theme::ColorfulTheme,
	) -> Result<(), Box<dyn Error>> {
		// Prompt for installation media
		{
			let iso_url: String = dialoguer::Input::with_theme(theme)
				.with_prompt("Enter the installation ISO URL")
				.interact()?;
		}

		// Prompt for minimal install
		if dialoguer::Confirm::with_theme(theme).with_prompt("Perform minimal install? This will remove as many unnecessary programs as possible.").interact()? {

		}

		// Prompt provisioners
		self.provisioners.prompt(config, theme)?;

		Ok(())
	}
}
