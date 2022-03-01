use crate::packer::QemuBuilder;
use crate::windows::Component;
use crate::windows::ComputerName;
use crate::windows::Settings;
use crate::windows::UnattendXml;
use crate::Config;

pub fn init(config: &mut Config) {
    config.user.username = String::from("admin");
    config.user.password = String::from("admin");
    config.iso_url = String::from("<ISO URL>");
    config.iso_checksum = String::from("<ISO checksum>");
}

pub fn default_builder() -> QemuBuilder {
    let mut builder = QemuBuilder::new();
    builder.boot_command = vec!["<enter>".into()];
    builder.boot_wait = "4s".into();
    builder.shutdown_command = "shutdown /s /t 0 /f /d p:4:1 /c \"Packer Shutdown\"".into();
    builder.communicator = "winrm".into();
    builder.winrm_insecure = Some(true);
    builder.winrm_timeout = Some("2h".into());
    builder.floppy_files = Some(vec!["Autounattend.xml".into()]);

    return builder;
}

pub fn unattended(config: &Config) -> UnattendXml {
    UnattendXml {
        xmlns: "urn:schemas-microsoft-com:unattend".into(),
        settings: vec![Settings {
            pass: "specialize".into(),
            component: vec![Component {
                name: "Microsoft-Windows-Shell-Setup".into(),
                processorArchitecture: "amd64".into(),
                publicKeyToken: "31bf3856ad364e35".into(),
                language: "neutral".into(),
                versionScope: "nonSxS".into(),
                ComputerName: Some(ComputerName {
                    value: config.name.clone(),
                }),
                DiskConfiguration: None,
                ImageInstall: None,
            }],
        }],
    }
}

pub fn build() {
    std::fs::write(
        "",
        r#"
			# Supress network location Prompt
			New-Item -Path "HKLM:\SYSTEM\CurrentControlSet\Control\Network\NewNetworkWindowOff" -Force

			# Set network to private
			$ifaceinfo = Get-NetConnectionProfile
			Set-NetConnectionProfile -InterfaceIndex $ifaceinfo.InterfaceIndex -NetworkCategory Private 

			# Configure WinRM itself
			winrm quickconfig -q
			winrm s "winrm/config" '@{MaxTimeoutms="1800000"}'
			winrm s "winrm/config/winrs" '@{MaxMemoryPerShellMB="2048"}'
			winrm s "winrm/config/service" '@{AllowUnencrypted="true"}'
			winrm s "winrm/config/service/auth" '@{Basic="true"}'

			# Enable the WinRM Firewall rule, which will likely already be enabled due to the 'winrm quickconfig' command above
			Enable-NetFirewallRule -DisplayName "Windows Remote Management (HTTP-In)"

			exit 0
		"#,
    );
}
