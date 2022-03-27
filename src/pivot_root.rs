use std::process::Command;

/// Check that the current system is a candidate for pivoting.
#[cfg(target_os = "linux")]
pub fn check_pivot_root() -> bool {
	false
}

fn exec_silent(exe: &str, args: &[&str]) {
	Command::new(exe)
        .args(args)
        .status()
        .expect("Failed to exec");
}

/// Pivot the root mountpoint to a temporary directory so we can reimage the root
/// partition on-the-fly. We have no intention of ever pivoting back, so it's OK
/// to break stuff. Taken from: https://unix.stackexchange.com/a/227318.
#[cfg(target_os = "linux")]
pub fn pivot_root() {

	// Unmount everything we can
	exec_silent("umount", &["-a"]);

    // Create a temporary root
    exec_silent("mkdir", &["/tmp/tmproot"]);
    exec_silent("mount", &["-t", "tmpfs", "none", "/tmp/tmproot"]);
    exec_silent("mkdir", &["/tmp/tmproot/proc"]);
    exec_silent("mkdir", &["/tmp/tmproot/sys"]);
    exec_silent("mkdir", &["/tmp/tmproot/dev"]);
    exec_silent("mkdir", &["/tmp/tmproot/run"]);
    exec_silent("mkdir", &["/tmp/tmproot/usr"]);
    exec_silent("mkdir", &["/tmp/tmproot/var"]);
    exec_silent("mkdir", &["/tmp/tmproot/tmp"]);
    exec_silent("mkdir", &["/tmp/tmproot/oldroot"]);
    exec_silent("cp", &["-ax", "/bin", "/tmp/tmproot/"]);
    exec_silent("cp", &["-ax", "/etc", "/tmp/tmproot/"]);
    exec_silent("cp", &["-ax", "/sbin", "/tmp/tmproot/"]);
    exec_silent("cp", &["-ax", "/lib", "/tmp/tmproot/"]);
    exec_silent("cp", &["-ax", "/lib64", "/tmp/tmproot/"]);

    // Run pivot root
    exec_silent("mount", &["--make-rprivate", "/"]);
    exec_silent("pivot_root", &["/tmp/tmproot", "/tmp/tmproot/oldroot"]);
    exec_silent("mount", &["--move", "/oldroot/dev", "/dev"]);
    exec_silent("mount", &["--move", "/oldroot/proc", "/proc"]);
    exec_silent("mount", &["--move", "/oldroot/sys", "/sys"]);
    exec_silent("mount", &["--move", "/oldroot/run", "/run"]);

    // Clean up processes holding onto files in the old root
    exec_silent("systemctl", &["daemon-reexec"]);
    exec_silent("fuser", &["-mk", "/oldroot"]);

    // Lastly unmount the original root filesystem
    exec_silent("umount", &["/oldroot"]);
}