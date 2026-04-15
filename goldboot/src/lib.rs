use rand::Rng;

use std::net::TcpListener;

#[cfg(feature = "build")]
pub mod builder;
#[cfg(feature = "cli")]
pub mod cli;
pub mod gpt;
#[cfg(feature = "gui")]
pub mod gui;
pub mod library;
pub mod registry;

/// Build info
pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

/// Find a random open TCP port in the given range.
pub fn find_open_port(lower: u16, upper: u16) -> u16 {
    loop {
        let port = rand::rng().random_range(lower..upper);
        match TcpListener::bind(format!("0.0.0.0:{port}")) {
            Ok(_) => break port,
            Err(_) => continue,
        }
    }
}

/// Returns whether a buffer of `size` bytes can fit in available memory (with a 2% safety buffer).
#[cfg(any(feature = "cli", feature = "gui"))]
pub fn can_preload(size: u64) -> bool {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    let avail = sys.available_memory();
    avail > 0 && size <= avail * 98 / 100
}

/// Generate a random password
pub fn random_password() -> String {
    // TODO check for a dictionary to generate something memorable

    // Fallback to random letters and numbers
    rand::rng()
        .sample_iter(&rand::distr::Alphanumeric)
        .take(12)
        .map(char::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_open_port() {
        let port = find_open_port(9000, 9999);

        assert!(port < 9999);
        assert!(port >= 9000);
    }
}
