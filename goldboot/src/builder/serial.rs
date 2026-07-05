//! Provides access to the serial console of a running VM. Waiting for text
//! over serial is much cheaper than running OCR against VNC screenshots, so
//! builders should prefer it whenever the guest can direct output to the
//! serial port.

use anyhow::{Result, bail};
use std::{
    io::Read,
    os::unix::net::UnixStream,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};
use tracing::{debug, trace};

/// Represents a connection to the serial console of a running VM. A background
/// thread continuously drains the socket into a buffer so guest output isn't
/// dropped while nobody is waiting on it.
#[derive(Clone)]
pub struct SerialConnection {
    buffer: Arc<Mutex<String>>,
}

impl SerialConnection {
    pub fn connect(socket_path: &Path) -> Result<Self> {
        debug!(socket = ?socket_path, "Connecting to serial console socket");
        let mut stream = UnixStream::connect(socket_path)?;

        let buffer = Arc::new(Mutex::new(String::new()));
        {
            let buffer = buffer.clone();

            // The thread exits once QEMU stops and closes the socket
            std::thread::spawn(move || {
                let mut chunk = [0u8; 4096];
                loop {
                    match stream.read(&mut chunk) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            let text = String::from_utf8_lossy(&chunk[..n]);
                            trace!("Serial console output: {}", &text);
                            buffer.lock().unwrap().push_str(&text);
                        }
                    }
                }
            });
        }

        Ok(Self { buffer })
    }

    /// Wait for text matching the given regex to appear on the serial console.
    /// The buffer is consumed through the end of the match so subsequent waits
    /// don't re-match old output.
    pub fn wait_for_match(&self, pattern: &str) -> Result<()> {
        let re = regex::Regex::new(pattern)?;
        debug!("Waiting for serial console text matching: {}", pattern);

        // Track the buffer size over time because we should exit if the guest
        // produces no output for a really long time.
        let mut running_len = 0;
        let mut running_count = 0;
        let mut total_count = 0u64;

        loop {
            {
                let mut buffer = self.buffer.lock().unwrap();
                if let Some(m) = re.find(buffer.as_str()) {
                    debug!(total_count, "Finished serial text wait");
                    let end = m.end();
                    buffer.drain(..end);
                    break;
                }

                if buffer.len() == running_len {
                    // TODO configurable
                    if running_count > 1200 {
                        // Include the most recent output to aid debugging
                        let mut start = buffer.len().saturating_sub(512);
                        while !buffer.is_char_boundary(start) {
                            start += 1;
                        }
                        bail!(
                            "No serial console output in 600 sec; buffer tail: {:?}",
                            &buffer[start..]
                        );
                    }
                    running_count += 1;
                } else {
                    running_len = buffer.len();
                    running_count = 0;
                }
            }

            total_count += 1;
            std::thread::sleep(Duration::from_millis(500));
        }
        Ok(())
    }
}
