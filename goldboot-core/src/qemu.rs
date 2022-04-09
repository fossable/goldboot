use crate::config::Config;
use crate::image_library_path;
use crate::ssh::SshConnection;
use crate::vnc::VncConnection;
use log::{debug, info};
use simple_error::bail;
use std::error::Error;
use std::path::Path;
use std::process::Child;
use std::process::Command;
use std::time::Duration;

/// Search filesystem for UEFI firmware
fn ovmf_firmware() -> Option<String> {
    for path in vec![
        "/usr/share/ovmf/x64/OVMF.fd",
        "/usr/share/OVMF/OVMF_CODE.fd",
    ] {
        if Path::new(&path).is_file() {
            debug!("Located OVMF firmware at: {}", path.to_string());
            return Some(path.to_string());
        }
    }

    debug!("Failed to locate OVMF firmware");
    None
}

/// Allocate a new temporary image.
pub fn allocate_image(config: &Config) -> Result<String, Box<dyn Error>> {
    let directory = image_library_path().join("tmp");
    std::fs::create_dir_all(&directory)?;

    let path = directory.join(&config.name);
    let path_string = path.to_string_lossy().to_string();

    debug!("Allocating new {} image: {}", config.disk_size, path_string);
    goldboot_image::Qcow2::create(&path, 256000000000, serde_json::to_vec(&config)?)?;
    Ok(path_string)
}

pub fn compact_qcow2(path: &str) -> Result<(), Box<dyn Error>> {
    let tmp_path = format!("{}.out", &path);

    info!("Compacting image");
    if let Some(code) = Command::new("qemu-img")
        .arg("convert")
        .arg("-c")
        .arg("-O")
        .arg("qcow2")
        .arg(&path)
        .arg(&tmp_path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("Failed to launch qemu-img")
        .code()
    {
        if code != 0 {
            bail!("qemu-img failed with error code: {}", code);
        } else {
            info!(
                "Reduced image size from {} to {}",
                std::fs::metadata(&path)?.len(),
                std::fs::metadata(&tmp_path)?.len()
            );

            // Replace the original before returning
            std::fs::rename(&tmp_path, &path)?;
            Ok(())
        }
    } else {
        panic!();
    }
}

pub struct QemuProcess {
    pub child: Child,
    pub vnc: VncConnection,
}

impl QemuProcess {
    pub fn new(args: &QemuArgs) -> Result<QemuProcess, Box<dyn Error>> {
        info!("Spawning new virtual machine");

        let cmdline = args.to_cmdline();
        debug!("QEMU arguments: {:?}", &cmdline);

        // Start the VM
        let mut child = Command::new(&args.exe)
            .args(cmdline.iter())
            .spawn()
            .unwrap();

        // Connect to VNC
        let vnc = loop {
            match VncConnection::new("localhost", args.vnc_port, args.record, args.debug) {
                Ok(vnc) => break Ok(vnc),
                Err(_) => {
                    // Check process
                    match child.try_wait() {
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

        Ok(Self { child, vnc })
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
        self.child.wait()?;
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
    pub fn new(config: &Config) -> Self {
        Self {
            bios: ovmf_firmware().unwrap(),
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
            display: if config.build_debug {
                String::from("gtk")
            } else {
                String::from("none")
            },
            memory: config.memory.clone(),
            name: config.name.clone(),
            smp: String::from("4,sockets=1,cores=4,threads=1"),
            netdev: vec![format!(
                "user,id=user.0,hostfwd=tcp::{}-:22",
                config.ssh_port.clone().unwrap()
            )],
            vnc: vec![format!(
                "127.0.0.1:{}",
                config.vnc_port.clone().unwrap() % 5900
            )],
            vnc_port: config.vnc_port.unwrap(),
            exe: if let Some(arch) = &config.arch {
                match arch.as_str() {
                    "x86_64" => String::from("qemu-system-x86_64"),
                    "aarch64" => String::from("qemu-system-aarch64"),
                    _ => String::from("qemu-system-x86_64"),
                }
            } else {
                String::from("qemu-system-x86_64")
            },
            usbdevice: vec![],
            record: config.build_record,
            debug: config.build_debug,
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
