use rand::Rng;

use std::net::TcpListener;

pub mod cli;
pub mod config;
pub mod builder;
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
