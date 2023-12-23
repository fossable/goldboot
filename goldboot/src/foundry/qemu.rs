use crate::foundry::{ssh::SshConnection, vnc::VncConnection, FoundryWorker};
use anyhow::bail;
use anyhow::Result;
use goldboot_image::ImageArch;
use log::{debug, info, trace};
use std::path::PathBuf;
use std::{
    process::{Child, Command},
    time::Duration,
};

/// Get the QEMU system binary for the current platform.
pub fn current_qemu_binary() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "qemu-system-x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "qemu-system-aarch64"
    } else {
        panic!("Unsupported platform");
    }
}

/// Detect the best acceleration type for the current hardware.
pub fn detect_accel() -> String {
    if std::env::var("CI").is_ok() {
        return String::from("tcg");
    }
    if cfg!(target_arch = "x86_64") {
        if cfg!(target_os = "linux") {
            match Command::new("grep")
                .arg("-Eq")
                .arg("vmx|svm|0xc0f")
                .arg("/proc/cpuinfo")
                .status()
            {
                Ok(status) => {
                    if let Some(code) = status.code() {
                        if code == 0 {
                            String::from("kvm")
                        } else {
                            String::from("tcg")
                        }
                    } else {
                        String::from("tcg")
                    }
                }
                Err(_) => String::from("tcg"),
            }
        } else {
            String::from("tcg")
        }
    } else {
        String::from("tcg")
    }
}

pub fn mimic_hardware() {}

/// Wraps a qemu process and provides easy access to VNC and SSH.
pub struct QemuProcess {
    pub process: Child,
    pub vnc: VncConnection,
}

impl Drop for QemuProcess {
    fn drop(&mut self) {
        self.process.kill().unwrap_or_default();
    }
}

impl QemuProcess {
    pub fn ssh(&mut self, port: u16, username: &str, password: &str) -> Result<SshConnection> {
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

    pub fn shutdown_wait(&mut self) -> Result<()> {
        info!("Waiting for shutdown");

        // Wait for QEMU to exit
        self.process.wait()?;
        debug!("Shutdown complete");
        Ok(())
    }
}

pub struct QemuArgs {
    pub bios: String,
    pub boot: String,
    pub cpu: Option<String>,
    pub device: Vec<String>,
    pub display: String,
    pub drive: Vec<String>,
    pub global: Vec<String>,
    pub machine: String,
    pub memory: String,
    pub name: String,
    pub netdev: Vec<String>,
    pub smbios: Option<String>,
    pub smp: String,
    pub usbdevice: Vec<String>,
    pub vnc: Vec<String>,
}

impl Into<Vec<String>> for QemuArgs {
    fn into(self) -> Vec<String> {
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
            String::from("-rtc"),
            String::from("base=utc"),
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

        trace!("QEMU cmdline: {:?}", &cmdline);
        cmdline
    }
}

pub struct QemuBuilder {
    args: QemuArgs,

    exe: PathBuf,
    vnc_port: u16,
    ssh_port: u16,
    debug: bool,
    record: bool,
}

impl QemuBuilder {
    pub fn new(worker: &FoundryWorker) -> Self {
        Self {
            args: QemuArgs {
                bios: worker.ovmf_path.clone(),
                boot: String::from("once=d"),
                cpu: None,
                device: vec![String::from("virtio-net,netdev=user.0")],

                // Bring up a graphical console in debug mode (linux only)
                display: if worker.debug && cfg!(target_os = "linux") {
                    String::from("gtk")
                } else {
                    String::from("none")
                },
                drive: vec![],

                // This seems to be necessary for the EFI variables to persist
                global: vec![String::from("driver=cfi.pflash01,property=secure,value=on")],
                machine: format!("type=pc,accel={}", detect_accel()),

                // Use the recommended memory amount from the config or use a default
                memory: match &worker.config.memory {
                    Some(memory) => memory.clone(),
                    None => String::from("4G"),
                },
                name: worker.config.name.clone(),
                netdev: vec![format!(
                    "user,id=user.0,hostfwd=tcp::{}-:22",
                    worker.ssh_port
                )],
                smbios: None,
                smp: String::from("4,sockets=1,cores=4,threads=1"),
                usbdevice: vec![],
                vnc: vec![format!("127.0.0.1:{}", worker.vnc_port % 5900)],
            },
            vnc_port: worker.vnc_port,
            ssh_port: worker.ssh_port,
            exe: match &worker.config.arch {
                ImageArch::Amd64 => "qemu-system-x86_64",
                ImageArch::Arm64 => "qemu-system-aarch64",
                _ => "qemu-system-x86_64",
            }
            .into(),
            debug: todo!(),
            record: todo!(),
        }
    }

    /// Append a "-drive" argument to the invocation.
    pub fn drive(mut self, arg: &str) -> Self {
        // TODO validate
        // arg.split(',')
        self.args.drive.push(arg.to_string());
        self
    }

    pub fn start(self) -> Result<QemuProcess> {
        info!("Spawning new qemu process");

        // Start the VM
        let cmdline: Vec<String> = self.args.into();
        let mut process = Command::new(&self.exe).args(cmdline.iter()).spawn()?;

        // Connect to VNC immediately
        let vnc = loop {
            match VncConnection::new("localhost", self.vnc_port, self.record, self.debug) {
                Ok(vnc) => break Ok(vnc),
                Err(_) => {
                    // Check process
                    match process.try_wait() {
                        Ok(Some(_)) => {
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
        Ok(QemuProcess { process, vnc })
    }
}
