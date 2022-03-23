use crate::config::Config;
use crate::image_library_path;
use crate::ssh::SshConnection;
use crate::vnc::VncConnection;
use log::debug;
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
            return Some(path.to_string());
        }
    }
    None
}

/// Allocate a new temporary image of the requested size.
pub fn allocate_image(size: &str) -> Result<String, Box<dyn Error>> {
    let directory = image_library_path().join("tmp");
    std::fs::create_dir_all(&directory)?;

    let path = directory.join("1234").to_string_lossy().to_string();
    if let Some(code) = Command::new("qemu-img")
        .arg("create")
        .arg("-f")
        .arg("qcow2")
        .arg(&path)
        .arg(size)
        .status()
        .expect("Failed to launch qemu-img")
        .code()
    {
        if code != 0 {
            bail!("qemu-img failed with error code: {}", code);
        } else {
            Ok(path)
        }
    } else {
        panic!();
    }
}

pub fn compact_image(path: &str) -> Result<(), Box<dyn Error>> {
    let tmp_path = format!("{}.out", &path);
    if let Some(code) = Command::new("qemu-img")
        .arg("convert")
        .arg("-c")
        .arg("-O")
        .arg("qcow2")
        .arg(&path)
        .arg(&tmp_path)
        .status()
        .expect("Failed to launch qemu-img")
        .code()
    {
        if code != 0 {
            bail!("qemu-img failed with error code: {}", code);
        } else {
            debug!(
                "Compacted image size from {} to {}",
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
    ssh: Option<SshConnection>,
}

impl QemuProcess {
    pub fn new(args: &QemuArgs) -> Result<QemuProcess, Box<dyn Error>> {
        let cmdline = args.to_cmdline();
        debug!("Starting QEMU: {:?}", &cmdline);

        // Start the VM
        let mut child = Command::new(&args.exe)
            .args(cmdline.iter())
            .spawn()
            .unwrap();

        // Connect to VNC
        let vnc = loop {
            match VncConnection::new("localhost", args.vnc_port) {
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

        Ok(Self {
            child,
            vnc,
            ssh: None,
        })
    }

    pub fn ssh(&self) -> Result<SshConnection, Box<dyn Error>> {
        //if let Some(ssh) = &self.ssh {
        //    Ok(ssh)
        //} else {
        Ok(SshConnection {})
        //}
    }

    pub fn shutdown(&mut self, command: &str) -> Result<(), Box<dyn Error>> {
        // Send shutdown command
        self.ssh()?.run(command)?;

        // Wait for process to exit
        self.child.wait().unwrap();

        Ok(())
    }
}

pub struct QemuArgs {
    pub bios: String,
    pub boot: String,
    pub device: Vec<String>,
    pub drive: Vec<String>,
    pub global: Vec<String>,
    pub machine: String,
    pub memory: String,
    pub name: String,
    pub netdev: Vec<String>,
    pub vnc: Vec<String>,

    pub exe: String,
    pub vnc_port: u16,
}

impl QemuArgs {
    pub fn new(config: &Config) -> Self {
        Self {
            bios: ovmf_firmware().unwrap(),
            boot: String::from("once=d"),
            device: vec![String::from("virtio-net,netdev=user.0")],
            drive: vec![],
            global: vec![String::from("driver=cfi.pflash01,property=secure,value=on")],
            machine: String::from("type=pc,accel=kvm"),
            memory: config.memory.clone(),
            name: config.name.clone(),
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
        }
    }

    pub fn add_drive(&mut self, path: &str, r#if: &str) {
        self.drive.push(format!(
            "file={},if={},cache=writeback,discard=ignore,format=qcow2",
            path, r#if
        ));
    }

    pub fn add_cdrom(&mut self, path: String) {
        self.drive.push(format!("file={},media=cdrom", path));
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
            String::from("gtk"),
        ];

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
