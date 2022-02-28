use crate::packer::QemuBuilder;
use crate::Config;

pub fn init(config: &mut Config) {
	config.user.username = String::from("root");
	config.user.password = String::from("root");
}

pub fn default_builder() -> QemuBuilder {
	QemuBuilder {
		boot_command: vec!["echo root:root | chpasswd<enter><wait2>".into(), "systemctl start sshd<enter>".into()],
    	boot_wait: "50s".into(),
    	communicator: "ssh".into(),
    	format: "qcow2".into(),
    	headless: true,
    	iso_checksum: Some("sha1:77a20dcd9d838398cebb2c7c15f46946bdc3855e".into()),
    	iso_url: Some("https://mirrors.edge.kernel.org/archlinux/iso/2021.10.01/archlinux-2021.10.01-x86_64.iso".into()),
    	output_directory: None,
    	qemuargs: None,
    	r#type: "qemu".into(),
    	shutdown_command: "poweroff".into(),
    	ssh_password: Some("root".into()),
    	ssh_username: Some("root".into()),
    	ssh_wait_timeout: Some("5m".into()),
    	vm_name: None,
    	winrm_insecure: None,
    	winrm_password: None,
    	winrm_timeout: None,
    	winrm_username: None,
	}
}
