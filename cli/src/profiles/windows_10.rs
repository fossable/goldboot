
pub fn base_template() -> PackerTemplate {
	let mut template = PackerTemplate::default();
	template.boot_command = vec!["<enter>".to_string()];
	template.boot_wait = "4s".to_string();
	template.shutdown_command = "shutdown /s /t 0 /f /d p:4:1 /c \"Packer Shutdown\"".to_string();
	template.communicator = "winrm".to_string();
	template.winrm_insecure = true;
	template.winrm_timeout = "2h".to_string();
	template.iso_url = "https://mirrors.edge.kernel.org/archlinux/iso/2021.10.01/archlinux-2021.10.01-x86_64.iso".to_string();
	template.iso_checksum = "sha1:77a20dcd9d838398cebb2c7c15f46946bdc3855e".to_string();

	return template;
}

pub fn unattended(config: &Config) -> UnattendXml {
	UnattendXml {
		settings: vec![
			Settings {
				pass: "specialize",
				component: vec![
					Component {
						name: "Microsoft-Windows-Shell-Setup".to_string(),
						ComputerName: ComputerName{
							value: config.name.clone(),
						}
					}
				]
			}
		]
	}
}