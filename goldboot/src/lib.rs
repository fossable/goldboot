

use rand::Rng;

use std::{net::TcpListener};



pub mod cli;
pub mod foundry;
#[cfg(feature = "gui")]
pub mod gui;
pub mod library;
pub mod registry;

/// Find a random open TCP port in the given range.
pub fn find_open_port(lower: u16, upper: u16) -> u16 {
    let mut rand = rand::thread_rng();

    loop {
        let port = rand.gen_range(lower..upper);
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
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
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
