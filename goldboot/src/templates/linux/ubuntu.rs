use crate::{
    build::BuildWorker,
    cache::{MediaCache, MediaFormat},
    provisioners::*,
    qemu::QemuArgs,
    templates::*,
};
use serde::{Deserialize, Serialize};
use std::error::Error;
use strum::{Display, EnumIter, IntoEnumIterator};
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Debug, EnumIter)]
pub enum UbuntuRelease {
    Jammy,
    Impish,
    Hirsute,
    Groovy,
    Focal,
    Eoan,
    Disco,
    Cosmic,
    Bionic,
    Artful,
}

impl Display for UbuntuRelease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match &self {
                UbuntuRelease::Jammy => "22.04 LTS (Jammy Jellyfish)",
                UbuntuRelease::Impish => "21.10     (Impish Indri)",
                UbuntuRelease::Hirsute => "21.04     (Hirsute Hippo)",
                UbuntuRelease::Groovy => "20.10     (Groovy Gorilla)",
            }
        )
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, EnumIter, Display)]
pub enum UbuntuEdition {
    Server,
    Desktop,
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct UbuntuTemplate {
    pub id: TemplateId,
    pub edition: UbuntuEdition,
    pub release: UbuntuRelease,

    pub root_password: String,

    pub iso: IsoSource,
    pub hostname: HostnameProvisioner,

    pub ansible: Option<Vec<AnsibleProvisioner>>,
}

impl Default for UbuntuTemplate {
    fn default() -> Self {
        Self {
            root_password: String::from("root"),
            iso: IsoContainer {
                url: format!(""),
                checksum: String::from("none"),
            },
            edition: UbuntuEdition::Desktop,
            release: UbuntuRelease::Jammy,
            general: GeneralContainer {
                base: TemplateBase::Ubuntu,
                storage_size: String::from("15 GiB"),
                ..Default::default()
            },
            provisioners: ProvisionersContainer::default(),
        }
    }
}

impl Template for UbuntuTemplate {
    fn build(&self, context: &BuildWorker) -> Result<(), Box<dyn Error>> {
        let mut qemuargs = QemuArgs::new(&context);

        qemuargs.drive.push(format!(
            "file={},if=virtio,cache=writeback,discard=ignore,format=qcow2",
            context.image_path
        ));
        qemuargs.drive.push(format!(
            "file={},media=cdrom",
            MediaCache::get(self.iso.url.clone(), &self.iso.checksum, MediaFormat::Iso)?
        ));

        // Start VM
        let mut qemu = qemuargs.start_process()?;

        // Send boot command
        #[rustfmt::skip]
		qemu.vnc.boot_command(vec![
		])?;

        // Wait for SSH
        let mut ssh = qemu.ssh_wait(context.ssh_port, "root", &self.root_password)?;

        // Run provisioners
        self.provisioners.run(&mut ssh)?;

        // Shutdown
        ssh.shutdown("poweroff")?;
        qemu.shutdown_wait()?;
        Ok(())
    }

    fn general(&self) -> GeneralContainer {
        self.general.clone()
    }
}

impl PromptMut for UbuntuTemplate {
    fn prompt(
        &mut self,
        config: &BuildConfig,
        theme: &dialoguer::theme::ColorfulTheme,
    ) -> Result<(), Box<dyn Error>> {
        // Prompt edition
        {
            let editions: Vec<UbuntuEdition> = UbuntuEdition::iter().collect();
            let edition_index = dialoguer::Select::with_theme(theme)
                .with_prompt("Choose Ubuntu edition")
                .default(0)
                .items(&editions)
                .interact()?;

            self.edition = editions[edition_index];
        }

        // Prompt release
        {
            let releases: Vec<UbuntuRelease> = UbuntuRelease::iter().collect();
            let release_index = dialoguer::Select::with_theme(theme)
                .with_prompt("Choose Ubuntu release")
                .default(0)
                .items(&releases)
                .interact()?;

            self.release = releases[release_index];
        }

        Ok(())
    }
}
