use anyhow::Result;
use std::io::IsTerminal;
use std::{
    cmp::min,
    io::{Read, Write},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub enum ProgressBar {
    /// A hashing operation
    Hash,

    /// An image conversion operation
    Convert,

    /// A download operation
    Download,

    /// An image write operation
    Write,
}

impl ProgressBar {
    fn create_progressbar(&self, len: u64) -> indicatif::ProgressBar {
        match self {
            ProgressBar::Hash => {
                let progress = indicatif::ProgressBar::new(len);
                progress.set_style(indicatif::ProgressStyle::default_bar().template("{spinner:.blue} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})").unwrap().progress_chars("=>-"));
                progress.enable_steady_tick(Duration::from_millis(50));
                progress
            }
            ProgressBar::Convert => {
                let progress = indicatif::ProgressBar::new(len);
                progress.set_style(indicatif::ProgressStyle::default_bar().template("{spinner:.yellow} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})").unwrap().progress_chars("=>-"));
                progress.enable_steady_tick(Duration::from_millis(50));
                progress
            }
            ProgressBar::Download => {
                let progress = indicatif::ProgressBar::new(len);
                progress.set_style(indicatif::ProgressStyle::default_bar().template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})").unwrap().progress_chars("=>-"));
                progress.enable_steady_tick(Duration::from_millis(50));
                progress
            }
            ProgressBar::Write => {
                let progress = indicatif::ProgressBar::new(len);
                progress.set_style(indicatif::ProgressStyle::default_bar().template("{spinner:.red} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})").unwrap().progress_chars("=>-"));
                progress.enable_steady_tick(Duration::from_millis(50));
                progress
            }
        }
    }

    pub fn new(&self, len: u64) -> Box<dyn Fn(u64)> {
        if !show_progress() {
            // No progress bar
            return Box::new(|_| {});
        }

        let progress = self.create_progressbar(len);
        Box::new(move |v| {
            if progress.position() + v >= len {
                progress.finish_and_clear();
            } else {
                progress.inc(v);
            }
        })
    }

    pub fn new_write(
        &self,
        total_clusters: usize,
        block_size: u64,
    ) -> Box<dyn Fn(usize, Option<bool>)> {
        let tty = std::io::stderr().is_terminal();

        // Window size for the rolling write-speed average.
        const WRITE_WINDOW: Duration = Duration::from_secs(1);

        struct State {
            clusters_done: usize,
            read_start: Instant,
            read_bytes: u64,
            write_bytes: u64,
            // Ring of (timestamp, bytes_written) samples used for the rolling window.
            write_samples: std::collections::VecDeque<(Instant, u64)>,
            last_log: Instant,
        }

        let state = Arc::new(Mutex::new(State {
            clusters_done: 0,
            read_start: Instant::now(),
            read_bytes: 0,
            write_bytes: 0,
            write_samples: std::collections::VecDeque::new(),
            last_log: Instant::now(),
        }));

        Box::new(move |_idx, event| {
            let mut s = state.lock().unwrap();
            match event {
                None => {
                    // Cluster is dirty, about to be written — record the read.
                    s.read_bytes += block_size;
                }
                Some(true) => {
                    // Dirty cluster written successfully.
                    s.write_bytes += block_size;
                    s.write_samples.push_back((Instant::now(), block_size));
                    s.clusters_done += 1;
                }
                Some(false) => {
                    // Clean cluster, only a read happened.
                    s.read_bytes += block_size;
                    s.clusters_done += 1;
                }
            }

            let done = s.clusters_done >= total_clusters;
            let should_print = done || tty || s.last_log.elapsed() >= Duration::from_secs(30);

            if should_print {
                let elapsed_read = s.read_start.elapsed().as_secs_f64().max(0.001);
                let read_speed = s.read_bytes as f64 / elapsed_read;

                // Evict samples older than WRITE_WINDOW, then sum the remainder.
                let cutoff = Instant::now() - WRITE_WINDOW;
                while s.write_samples.front().is_some_and(|(t, _)| *t < cutoff) {
                    s.write_samples.pop_front();
                }
                let window_bytes: u64 = s.write_samples.iter().map(|(_, b)| b).sum();
                let write_speed = if window_bytes > 0 {
                    window_bytes as f64 / WRITE_WINDOW.as_secs_f64()
                } else {
                    0.0
                };

                if tty {
                    if done {
                        eprintln!(
                            "\rRead: {}/s  Write: {}/s  Written: {}    ",
                            fmt_bytes(read_speed),
                            fmt_bytes(write_speed),
                            fmt_bytes_precise(s.write_bytes as f64),
                        );
                    } else {
                        eprint!(
                            "\rRead: {}/s  Write: {}/s  Written: {}    ",
                            fmt_bytes(read_speed),
                            fmt_bytes(write_speed),
                            fmt_bytes_precise(s.write_bytes as f64),
                        );
                        let _ = std::io::stderr().flush();
                    }
                } else {
                    eprintln!(
                        "deploy: read {}/s  write {}/s  written {}",
                        fmt_bytes(read_speed),
                        fmt_bytes(write_speed),
                        fmt_bytes_precise(s.write_bytes as f64),
                    );
                    s.last_log = Instant::now();
                }
            }
        })
    }

    /// Fully copy the given reader to the given writer and display a
    /// progressbar if running in interactive mode.
    pub fn copy(&self, reader: &mut dyn Read, writer: &mut dyn Write, len: u64) -> Result<()> {
        if !show_progress() {
            // No progress bar
            std::io::copy(reader, writer)?;
            return Ok(());
        }

        let progress = self.create_progressbar(len);

        let mut buffer = [0u8; 1024 * 1024];
        let mut copied: u64 = 0;

        loop {
            if let Ok(size) = reader.read(&mut buffer) {
                if size == 0 {
                    break;
                }
                writer.write(&buffer[0..size])?;
                let new = min(copied + (size as u64), len);
                copied = new;
                progress.set_position(new);
            } else {
                break;
            }
        }

        progress.finish_and_clear();
        Ok(())
    }
}
fn show_progress() -> bool {
    std::io::stdout().is_terminal() && !std::env::var("CI").is_ok()
}

fn fmt_bytes(bytes: f64) -> String {
    use byte_unit::{Byte, UnitType};
    let unit = Byte::from_u64(bytes as u64).get_appropriate_unit(UnitType::Binary);
    format!("{:>4.0} {}", unit.get_value(), unit.get_unit())
}

fn fmt_bytes_precise(bytes: f64) -> String {
    use byte_unit::{Byte, UnitType};
    let unit = Byte::from_u64(bytes as u64).get_appropriate_unit(UnitType::Binary);
    format!("{:.2} {}", unit.get_value(), unit.get_unit())
}
