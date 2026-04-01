use goldboot_image::ImageHandle;
use std::{
    io::IsTerminal,
    path::Path,
    process::ExitCode,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tracing::error;

pub fn run(cmd: super::Commands) -> ExitCode {
    match cmd {
        super::Commands::Drift {
            image,
            input: device,
        } => {
            let mut image_handle = if Path::new(&image).exists() {
                match ImageHandle::open(&image) {
                    Ok(h) => h,
                    Err(err) => {
                        error!(error = ?err, "Failed to open image");
                        return ExitCode::FAILURE;
                    }
                }
            } else {
                match crate::library::ImageLibrary::find_by_id(&image) {
                    Ok(h) => h,
                    Err(err) => {
                        error!(error = ?err, "Image not found");
                        return ExitCode::FAILURE;
                    }
                }
            };
            if image_handle.load(None).is_err() {
                return ExitCode::FAILURE;
            }

            let device_path = Path::new(&device);
            if !device_path.exists() {
                error!("Device '{}' does not exist", device);
                return ExitCode::FAILURE;
            }

            let total_clusters = image_handle
                .protected_header
                .as_ref()
                .map(|h| h.cluster_count as usize)
                .unwrap_or(0);
            let block_size = image_handle
                .protected_header
                .as_ref()
                .map(|h| h.block_size as u64)
                .unwrap_or(0);

            let tty = std::io::stderr().is_terminal();
            const READ_WINDOW: Duration = Duration::from_secs(1);

            struct State {
                clusters_done: usize,
                read_start: Instant,
                read_bytes: u64,
                mismatches: usize,
                read_samples: std::collections::VecDeque<(Instant, u64)>,
                last_log: Instant,
            }

            let state = Arc::new(Mutex::new(State {
                clusters_done: 0,
                read_start: Instant::now(),
                read_bytes: 0,
                mismatches: 0,
                read_samples: std::collections::VecDeque::new(),
                last_log: Instant::now(),
            }));

            let progress = {
                let state = Arc::clone(&state);
                move |_idx: usize, event: Option<bool>| {
                    let mut s = state.lock().unwrap();
                    match event {
                        None => {
                            // Cluster is being read/hashed.
                            s.read_bytes += block_size;
                            s.read_samples.push_back((Instant::now(), block_size));
                        }
                        Some(true) => {
                            // Hash matched.
                            s.clusters_done += 1;
                        }
                        Some(false) => {
                            // Hash mismatch.
                            s.clusters_done += 1;
                            s.mismatches += 1;
                        }
                    }

                    let done = s.clusters_done >= total_clusters;
                    let should_print =
                        done || tty || s.last_log.elapsed() >= Duration::from_secs(30);

                    if should_print {
                        let elapsed = s.read_start.elapsed().as_secs_f64().max(0.001);
                        let avg_speed = s.read_bytes as f64 / elapsed;

                        // Rolling window read speed.
                        let cutoff = Instant::now() - READ_WINDOW;
                        while s.read_samples.front().is_some_and(|(t, _)| *t < cutoff) {
                            s.read_samples.pop_front();
                        }
                        let window_bytes: u64 = s.read_samples.iter().map(|(_, b)| b).sum();
                        let window_speed = if window_bytes > 0 {
                            window_bytes as f64 / READ_WINDOW.as_secs_f64()
                        } else {
                            avg_speed
                        };

                        use byte_unit::{Byte, UnitType};
                        let fmt = |b: f64| {
                            let unit =
                                Byte::from_u64(b as u64).get_appropriate_unit(UnitType::Binary);
                            format!("{:>4.0} {}", unit.get_value(), unit.get_unit())
                        };
                        let fmt_precise = |b: f64| {
                            let unit =
                                Byte::from_u64(b as u64).get_appropriate_unit(UnitType::Binary);
                            format!("{:.2} {}", unit.get_value(), unit.get_unit())
                        };

                        if tty {
                            use std::io::Write as _;
                            if done {
                                eprintln!(
                                    "\rRead: {}/s  Read total: {}  Mismatches: {}    ",
                                    fmt(window_speed),
                                    fmt_precise(s.read_bytes as f64),
                                    s.mismatches,
                                );
                            } else {
                                eprint!(
                                    "\rRead: {}/s  Read total: {}  Mismatches: {}    ",
                                    fmt(window_speed),
                                    fmt_precise(s.read_bytes as f64),
                                    s.mismatches,
                                );
                                let _ = std::io::stderr().flush();
                            }
                        } else {
                            eprintln!(
                                "drift: read {}/s  read total {}  mismatches {}",
                                fmt(window_speed),
                                fmt_precise(s.read_bytes as f64),
                                s.mismatches,
                            );
                            s.last_log = Instant::now();
                        }
                    }
                }
            };

            if let Err(err) = image_handle.verify(device_path, progress) {
                error!(error = ?err, "Verification failed");
                return ExitCode::FAILURE;
            }

            ExitCode::SUCCESS
        }
        _ => panic!(),
    }
}
