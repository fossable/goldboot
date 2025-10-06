use crate::{
    enter,
    builder::{FoundryWorker, ssh::SshConnection, vnc::VncConnection},
};
use anyhow::{Result, bail};
use goldboot_image::ImageArch;
use rand::Rng;
use std::{
    collections::HashMap,
    fs::OpenOptions,
    io::Write,
    path::PathBuf,
    process::{Child, Command},
    time::Duration,
};
use strum::Display;
use tracing::{debug, info, trace};

use super::sources::{ImageSource, SourceCache};

#[derive(Display, Clone, Copy)]
pub enum OsCategory {
    Darwin,
    Linux,
    Windows,
}

impl OsCategory {
    /// Convert to String representation for use in a Github release asset
    pub fn as_github_string(&self) -> String {
        match self {
            OsCategory::Darwin => "apple",
            OsCategory::Linux => "linux",
            OsCategory::Windows => "windows",
        }
        .to_string()
    }
}

/// Supported VM hardware acceleration.
pub enum Accel {
    /// "Kernel VM" which requires Intel VT or AMD-V
    Kvm,
    /// Basically means no acceleration
    Tcg,
}

impl std::fmt::Display for Accel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match &self {
                Accel::Tcg => "tcg",
                Accel::Kvm => "kvm",
            }
        )
    }
}

/// Detect the best acceleration type for the current hardware.
pub fn detect_accel() -> Accel {
    if std::env::var("CI").is_ok() {
        return Accel::Tcg;
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
                        if code == 0 { Accel::Kvm } else { Accel::Tcg }
                    } else {
                        Accel::Tcg
                    }
                }
                Err(_) => Accel::Tcg,
            }
        } else {
            Accel::Tcg
        }
    } else {
        Accel::Tcg
    }
}

pub fn mimic_hardware() {}

/// Wraps a qemu process and provides easy access to VNC and SSH.
pub struct QemuProcess {
    pub arch: ImageArch,
    pub process: Child,
    pub tpm_process: Option<Child>,
    pub ssh_port: u16,
    pub private_key: PathBuf,
    pub host_key: PathBuf,
    pub vnc: VncConnection,
    pub os_category: OsCategory,
}

impl Drop for QemuProcess {
    fn drop(&mut self) {
        debug!("Stopping Qemu process");
        self.process.kill().unwrap_or_default();

        // Kill TPM emulator after the Qemu process
        if let Some(tpm_process) = self.tpm_process.as_mut() {
            debug!("Stopping TPM emulator");
            tpm_process.kill().unwrap_or_default();
        }
    }
}

impl QemuProcess {
    pub fn ssh(&mut self, username: &str) -> Result<SshConnection> {
        #[rustfmt::skip]
        self.vnc.run(vec![
            // Mount the prepared filesystem
            enter!("mkdir /tmp/goldboot"),
            enter!("mount -t vfat /dev/vdb /tmp/goldboot"),

            // Spawn the temporary SSH server
            enter!(format!("/tmp/goldboot/sshdog {} /tmp/goldboot/host_key /tmp/goldboot/public_key", self.ssh_port)),
        ])?;

        Ok(SshConnection::new(
            username,
            &self.private_key,
            self.ssh_port,
        )?)
    }

    pub fn shutdown_wait(mut self) -> Result<()> {
        info!("Waiting for shutdown");

        // Wait for QEMU to exit
        self.process.wait()?;
        debug!("Shutdown complete");
        Ok(())
    }
}

#[derive(Debug)]
pub struct QemuArgs {
    pub bios: String,
    pub blockdev: Vec<String>,
    pub boot: String,
    pub chardev: Vec<String>,
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
    pub tpmdev: Vec<String>,
    pub usbdevice: Vec<String>,
    pub vga: String,
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

        for blockdev in &self.blockdev {
            cmdline.push(String::from("-blockdev"));
            cmdline.push(blockdev.to_string());
        }

        for chardev in &self.chardev {
            cmdline.push(String::from("-chardev"));
            cmdline.push(chardev.to_string());
        }

        for tpmdev in &self.tpmdev {
            cmdline.push(String::from("-tpmdev"));
            cmdline.push(tpmdev.to_string());
        }

        for device in &self.device {
            cmdline.push(String::from("-device"));
            cmdline.push(device.to_string());
        }

        cmdline.push(String::from("-vga"));
        cmdline.push(self.vga.to_string());

        trace!("QEMU cmdline: {:?}", &cmdline);
        cmdline
    }
}

pub struct QemuBuilder {
    arch: ImageArch,
    args: QemuArgs,
    debug: bool,
    record: bool,
    ssh_port: u16,
    ssh_private_key: PathBuf,
    ssh_host_key: PathBuf,
    vnc_port: u16,
    temp: PathBuf,
    os_category: OsCategory,
}

impl QemuBuilder {
    pub fn new(worker: &FoundryWorker, os_category: OsCategory) -> Self {
        let ssh_port = rand::rng().random_range(10000..11000);
        let ssh_private_key = crate::builder::ssh::generate_key(worker.tmp.path()).unwrap();
        let ssh_host_key = crate::builder::ssh::generate_key(worker.tmp.path()).unwrap();

        Self {
            args: QemuArgs {
                bios: worker.ovmf_path.display().to_string(),
                blockdev: vec![],
                chardev: vec![],
                tpmdev: vec![],
                boot: String::from("once=d"),
                cpu: None,
                device: vec![String::from("virtio-net,netdev=user.0")],

                // Bring up a graphical console in debug mode (linux only)
                display: if worker.debug && cfg!(target_os = "linux") {
                    String::from("gtk")
                } else {
                    String::from("none")
                },

                // Add the output image as a drive
                // TODO nvme?
                drive: vec![format!(
                    "file={},if={},cache=writeback,discard=ignore,format=qcow2",
                    worker.qcow_path.display(),
                    match os_category {
                        OsCategory::Darwin => "virtio",
                        OsCategory::Linux => "virtio",
                        OsCategory::Windows => "ide",
                    },
                )],

                // This seems to be necessary for the EFI variables to persist
                global: vec![String::from("driver=cfi.pflash01,property=secure,value=on")],
                machine: format!("type=pc,accel={}", worker.accel),

                // Use the recommended memory amount from the config or use a default
                memory: worker.memory.clone(),
                name: String::from("goldboot"),
                netdev: vec!["user,id=user.0".into()],
                smbios: None,
                smp: String::from("4,sockets=1,cores=4,threads=1"),
                usbdevice: vec![],
                vnc: vec![format!("127.0.0.1:{}", worker.vnc_port % 5900)],
                vga: String::from("std"),
            },
            arch: worker.arch,
            debug: worker.debug,
            os_category,
            record: worker.record,
            ssh_port,
            ssh_private_key,
            ssh_host_key,
            temp: worker.tmp.path().to_path_buf(),
            vnc_port: worker.vnc_port,
        }
    }

    /// Set the image source.
    pub fn source(mut self, source: &ImageSource) -> Result<Self> {
        match source {
            ImageSource::Iso { url, checksum } => {
                self.args.drive.push(format!(
                    "file={},media=cdrom,read-only=on",
                    SourceCache::default()?.get(url.clone(), checksum.clone())?
                ));
            }
            _ => todo!(),
        }

        Ok(self)
    }

    /// Append a "-drive" argument to the invocation.
    pub fn drive(mut self, arg: &str) -> Self {
        // TODO validate
        // arg.split(',')
        self.args.drive.push(arg.to_string());
        self
    }

    /// Update -vga
    pub fn vga(mut self, arg: &str) -> Self {
        self.args.vga = arg.to_string();
        self
    }

    /// Update -cpu
    pub fn cpu(mut self, arg: &str) -> Self {
        self.args.cpu = Some(arg.to_string());
        self
    }

    /// Create a temporary FAT filesystem with the given contents and append it
    /// to the invocation.
    pub fn drive_files(mut self, files: HashMap<String, Vec<u8>>) -> Result<Self> {
        let fs_name: String = rand::rng()
            .sample_iter(&rand::distr::Alphanumeric)
            .take(12)
            .map(char::from)
            .collect();
        let fs_path = self.temp.join(fs_name);

        // Add a buffer of extra space
        let mut fs_size: u64 = files.values().map(|c| c.len() as u64).sum();
        fs_size += 320000;

        debug!(
            fs_path = ?fs_path,
            fs_size, "Formatting FAT filesystem"
        );
        {
            let fs_file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(&fs_path)?;
            fs_file.set_len(fs_size)?;

            fatfs::format_volume(
                fscommon::BufStream::new(fs_file),
                fatfs::FormatVolumeOptions::new(),
            )?;

            let fs_file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(&fs_path)?;
            let fs = fatfs::FileSystem::new(fs_file, fatfs::FsOptions::new())?;
            let root_dir = fs.root_dir();

            for (path, content) in &files {
                debug!(path = ?path, size = content.len(), "Copying file to temporary filesystem");
                let mut file = root_dir.create_file(path)?;
                file.write_all(content)?;
            }
        }

        self.args.drive.push(format!(
            "file={},if=virtio,cache=writeback,discard=ignore,format=raw",
            fs_path.display()
        ));
        Ok(self)
    }

    /// Create a temporary FAT filesystem with the given contents and append it
    /// to the invocation.
    pub fn floppy_files(mut self, files: HashMap<String, Vec<u8>>) -> Result<Self> {
        const FLOPPY_SIZE: u64 = 1474560;

        let fs_name: String = rand::rng()
            .sample_iter(&rand::distr::Alphanumeric)
            .take(12)
            .map(char::from)
            .collect();
        let fs_path = self.temp.join(fs_name);

        let fs_size: u64 = files.values().map(|c| c.len() as u64).sum();
        if fs_size > FLOPPY_SIZE {
            bail!("Too large for floppy drive");
        }

        debug!(
            fs_path = ?fs_path,
            fs_size, "Formatting FAT filesystem"
        );
        {
            let fs_file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(&fs_path)?;

            fs_file.set_len(FLOPPY_SIZE)?;

            fatfs::format_volume(
                fscommon::BufStream::new(fs_file),
                fatfs::FormatVolumeOptions::new()
                    .fat_type(fatfs::FatType::Fat12)
                    .sectors_per_track(18),
            )?;

            let fs_file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(&fs_path)?;
            let fs = fatfs::FileSystem::new(fs_file, fatfs::FsOptions::new())?;
            let root_dir = fs.root_dir();

            for (path, content) in &files {
                let mut file = root_dir.create_file(path)?;
                file.write_all(content)?;
            }
        }

        // TODO dynamic f0
        self.args.device.push("floppy,drive=f0".to_string());
        self.args.blockdev.push(format!(
            "driver=file,node-name=f0,filename={}",
            fs_path.display()
        ));
        Ok(self)
    }

    pub fn prepare_ssh(mut self) -> Result<Self> {
        let sshdog = crate::builder::ssh::download_sshdog(self.arch, self.os_category)?;
        let host_key = std::fs::read(&self.ssh_host_key)?;
        let public_key = std::fs::read(self.ssh_private_key.with_extension("pub"))?;

        self.args.netdev.truncate(0);
        self.args.netdev.push(format!(
            "user,id=user.0,hostfwd=tcp::{}-:{}",
            self.ssh_port, self.ssh_port
        ));

        Ok(self.drive_files(HashMap::from([
            ("sshdog".to_string(), sshdog),
            ("host_key".to_string(), host_key),
            ("public_key".to_string(), public_key),
        ]))?)
    }

    /// Enable TPM emulation.
    pub fn enable_tpm(mut self) -> Result<Self> {
        // TODO skip if swtpm isn't installed
        self.args.chardev.push(format!(
            "socket,id=chrtpm,path={}/tpm.sock",
            self.temp.display()
        ));
        self.args
            .tpmdev
            .push("emulator,id=tpm0,chardev=chrtpm".into());
        self.args.device.push("tpm-tis,tpmdev=tpm0".into());
        Ok(self)
    }

    pub fn start(self) -> Result<QemuProcess> {
        // Start the TPM emulator if one was requested
        let tpm_process = if self.args.tpmdev.len() > 0 {
            let args = vec![
                "socket".to_string(),
                "--tpmstate".to_string(),
                format!("dir={}", self.temp.display()),
                "--ctrl".to_string(),
                format!("type=unixio,path={}/tpm.sock", self.temp.display()),
                "--tpm2".to_string(),
            ];

            info!(args = ?args, "Spawning new TPM emulator process");
            Some(Command::new("swtpm").args(args).spawn()?)
        } else {
            None
        };

        info!(args = ?self.args, "Spawning new qemu process");

        // Start the VM
        let cmdline: Vec<String> = self.args.into();
        let mut process = Command::new(match &self.arch {
            ImageArch::Amd64 => "qemu-system-x86_64",
            ImageArch::Arm64 => "qemu-system-aarch64",
            _ => bail!("Unknown arch"),
        })
        .args(cmdline.iter())
        .spawn()?;

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
        Ok(QemuProcess {
            arch: self.arch,
            process,
            tpm_process,
            ssh_port: self.ssh_port,
            private_key: self.ssh_private_key,
            host_key: self.ssh_host_key,
            vnc,
            os_category: self.os_category,
        })
    }
}
