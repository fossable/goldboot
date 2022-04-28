use crate::{build::BuildWorker, ssh::SshConnection, vnc::VncConnection};
use log::{debug, info};
use simple_error::bail;
use std::error::Error;

use std::{
	process::{Child, Command},
	time::Duration,
};

/// Get the QEMU system binary for the current platform
pub fn current_qemu_binary() -> &'static str {
	if cfg!(target_arch = "x86_64") {
		"qemu-system-x86_64"
	} else if cfg!(target_arch = "aarch64") {
		"qemu-system-aarch64"
	} else {
		panic!("Unsupported platform");
	}
}

pub struct QemuProcess {
	pub process: Child,
	pub vnc: VncConnection,
}

impl QemuProcess {
	pub fn new(args: &QemuArgs) -> Result<QemuProcess, Box<dyn Error>> {
		info!("Spawning new virtual machine");

		let cmdline = args.to_cmdline();
		debug!("QEMU arguments: {:?}", &cmdline);

		// Start the VM
		let mut process = Command::new(&args.exe)
			.args(cmdline.iter())
			.spawn()
			.unwrap();

		// Connect to VNC
		let vnc = loop {
			match VncConnection::new("localhost", args.vnc_port, args.record, args.debug) {
				Ok(vnc) => break Ok(vnc),
				Err(_) => {
					// Check process
					match process.try_wait() {
						Ok(Some(status)) => {
							bail!("Qemu exited early")
						}
						Ok(None) => {
							// Wait before trying again
							std::thread::sleep(Duration::from_secs(5));
						}
						Err(e) => break Err(e),
					}
				}
			}
		}?;

		Ok(Self { process, vnc })
	}

	pub fn ssh_wait(
		&mut self,
		port: u16,
		username: &str,
		password: &str,
	) -> Result<SshConnection, Box<dyn Error>> {
		info!("Waiting for SSH connection");

		let mut i = 0;

		Ok(loop {
			i += 1;
			std::thread::sleep(Duration::from_secs(5));

			match SshConnection::new(port, &username, &password) {
				Ok(ssh) => break ssh,
				Err(error) => debug!("{}", error),
			}

			if i > 25 {
				bail!("Maximum iterations reached");
			}
		})
	}

	pub fn shutdown_wait(&mut self) -> Result<(), Box<dyn Error>> {
		info!("Waiting for shutdown");

		// Wait for QEMU to exit
		self.process.wait()?;
		Ok(())
	}
}

pub struct QemuArgs {
	pub bios: String,
	pub boot: String,
	pub cpu: Option<String>,
	pub device: Vec<String>,
	pub drive: Vec<String>,
	pub display: String,
	pub global: Vec<String>,
	pub machine: String,
	pub memory: String,
	pub name: String,
	pub netdev: Vec<String>,
	pub vnc: Vec<String>,
	pub smp: String,
	pub smbios: Option<String>,
	pub usbdevice: Vec<String>,

	pub exe: String,
	pub vnc_port: u16,
	pub record: bool,
	pub debug: bool,
}

impl QemuArgs {
	pub fn new(context: &BuildWorker) -> Self {
		Self {
			bios: context.ovmf_path.clone(),
			boot: String::from("once=d"),
			cpu: None,
			smbios: None,
			device: vec![String::from("virtio-net,netdev=user.0")],
			drive: vec![],
			global: vec![String::from("driver=cfi.pflash01,property=secure,value=on")],
			machine: if std::env::var("CI").is_ok() {
				String::from("type=pc")
			} else {
				String::from("type=pc,accel=kvm")
			},
			display: if context.debug {
				String::from("gtk")
			} else {
				String::from("none")
			},
			memory: context.config.memory.clone(),
			name: context.config.name.clone(),
			smp: String::from("4,sockets=1,cores=4,threads=1"),
			netdev: vec![format!(
				"user,id=user.0,hostfwd=tcp::{}-:22",
				context.ssh_port
			)],
			vnc: vec![format!("127.0.0.1:{}", context.vnc_port % 5900)],
			vnc_port: context.vnc_port,
			exe: if let Some(arch) = &context.config.arch {
				match arch.as_str() {
					"x86_64" => String::from("qemu-system-x86_64"),
					"aarch64" => String::from("qemu-system-aarch64"),
					_ => String::from("qemu-system-x86_64"),
				}
			} else {
				String::from("qemu-system-x86_64")
			},
			usbdevice: vec![],
			record: context.record,
			debug: context.debug,
		}
	}

	pub fn to_cmdline(&self) -> Vec<String> {
		let mut cmdline = vec![
			String::from("-name"),
			self.name.clone(),
			String::from("-bios"),
			self.bios.clone(),
			String::from("-m"),
			self.memory.clone(),
			String::from("-boot"),
			self.boot.clone(),
			String::from("-display"),
			self.display.clone(),
			String::from("-smp"),
			self.smp.clone(),
			String::from("-machine"),
			self.machine.clone(),
		];

		if let Some(cpu) = &self.cpu {
			cmdline.push(String::from("-cpu"));
			cmdline.push(cpu.clone());
		}

		if let Some(smbios) = &self.smbios {
			cmdline.push(String::from("-smbios"));
			cmdline.push(smbios.clone());
		}

		for usbdevice in &self.usbdevice {
			cmdline.push(String::from("-usbdevice"));
			cmdline.push(usbdevice.clone());
		}

		for global in &self.global {
			cmdline.push(String::from("-global"));
			cmdline.push(global.to_string());
		}

		for drive in &self.drive {
			cmdline.push(String::from("-drive"));
			cmdline.push(drive.to_string());
		}

		for netdev in &self.netdev {
			cmdline.push(String::from("-netdev"));
			cmdline.push(netdev.to_string());
		}

		for vnc in &self.vnc {
			cmdline.push(String::from("-vnc"));
			cmdline.push(vnc.to_string());
		}

		for device in &self.device {
			cmdline.push(String::from("-device"));
			cmdline.push(device.to_string());
		}

		cmdline
	}

	pub fn start_process(&self) -> Result<QemuProcess, Box<dyn Error>> {
		QemuProcess::new(self)
	}
}
