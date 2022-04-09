use indicatif::ProgressBar;
use indicatif::ProgressStyle;
use log::{debug, info};
use rand::Rng;
use sha1::{Digest, Sha1};
use simple_error::bail;
use std::error::Error;
use std::fs::File;
use std::io::BufWriter;
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;
use vnc::client::Event;

pub struct VncScreenshot {
    pub data: Vec<u8>,
    pub width: u16,
    pub height: u16,
}

impl VncScreenshot {
    pub fn hash(&self) -> String {
        hex::encode(Sha1::new().chain_update(&self.data).finalize())
    }

    pub fn write_png(&self, output_path: &Path) -> Result<(), Box<dyn Error>> {
        std::fs::create_dir_all(output_path.parent().unwrap())?;
        let ref mut w = BufWriter::new(File::create(output_path)?);

        let mut encoder = png::Encoder::new(w, self.width as u32, self.height as u32);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&self.data).unwrap();

        debug!(
            "Saved screenshot to: {:?}",
            std::fs::canonicalize(output_path)?
        );
        Ok(())
    }

    /// Compute a percentage of how similar the given screenshot is to this one.
    pub fn similarity(&self, _other: &VncScreenshot) -> f32 {
        todo!();
    }

    /// Create a trimmed screenshot according to the given dimensions
    pub fn trim(&self, rect: vnc::Rect) -> Result<VncScreenshot, Box<dyn Error>> {
        let w = rect.width as usize;
        let h = rect.height as usize;
        let mut data = vec![0u8; w * h];

        // TODO use copy_from_slice instead
        for y in 0..rect.height as usize {
            for x in 0..rect.width as usize {
                data[y * w + x] = self.data
                    [(y + rect.top as usize) * self.width as usize + (x + rect.left as usize)];
            }
        }

        Ok(VncScreenshot {
            data,
            width: rect.width,
            height: rect.height,
        })
    }
}

#[derive(Debug, Clone)]
pub enum Cmd {
    Enter,
    Spacebar,
    Tab,

    /// Input the left super button.
    LeftSuper,

    /// Input the given text characters with a half-second delay between each.
    Type(String),

    /// Wait the given amount of seconds.
    Wait(u64),

    /// Wait for the screen to match the given hash.
    WaitScreen(String),

    /// Wait for a section of the screen to match the given hash.
    WaitScreenRect(String, vnc::Rect),
}

pub struct VncConnection {
    pub width: u16,
    pub height: u16,
    pub vnc: vnc::Client,
    pub record: bool,
    pub debug: bool,
}

impl VncConnection {
    pub fn new(
        host: &str,
        port: u16,
        record: bool,
        debug: bool,
    ) -> Result<VncConnection, Box<dyn Error>> {
        debug!("Attempting VNC connection to: {}:{}", host, port);

        let mut vnc =
            vnc::Client::from_tcp_stream(TcpStream::connect((host, port))?, true, |_| {
                Some(vnc::client::AuthChoice::None)
            })?;

        let (width, height) = vnc.size();

        vnc.set_format(vnc::PixelFormat {
            bits_per_pixel: 8,
            depth: 8,
            big_endian: false,
            true_colour: true,
            red_max: 8,
            green_max: 8,
            blue_max: 4,
            red_shift: 5,
            green_shift: 2,
            blue_shift: 0,
        })?;
        vnc.set_encodings(&[vnc::Encoding::Raw, vnc::Encoding::DesktopSize])?;

        debug!("Connected to VNC ({} x {})", width, height);

        Ok(Self {
            width,
            height,
            vnc: vnc,
            record,
            debug,
        })
    }

    pub fn screenshot(&mut self) -> Result<VncScreenshot, Box<dyn Error>> {
        // Attempt to clear the framebuffer, but don't discard any resize events
        for event in self.vnc.poll_iter() {
            match event {
                Event::Resize(width, height) => {
                    self.width = width;
                    self.height = height;
                }
                _ => {}
            }
        }

        loop {
            let request_rect = vnc::Rect {
                left: 0,
                top: 0,
                width: self.width,
                height: self.height,
            };

            // Request a full screen update
            self.vnc.request_update(request_rect, false).unwrap();

            for event in self.vnc.poll_iter() {
                match event {
                    Event::Resize(width, height) => {
                        self.width = width;
                        self.height = height;
                    }
                    Event::PutPixels(vnc_rect, ref pixels) => {
                        if vnc_rect == request_rect {
                            return Ok(VncScreenshot {
                                width: vnc_rect.width,
                                height: vnc_rect.height,
                                data: pixels.clone(),
                            });
                        }
                    }
                    Event::EndOfFrame => {}
                    _ => bail!("VNC poll failed"),
                }
            }
        }
    }

    fn handle_breakpoint(&mut self, cmd: &Cmd) -> Result<(), Box<dyn Error>> {
        loop {
            info!(
                "(breakpoint)['c' to continue, 's' to screenshot, 'q' to quit debugging] Next command: {:?}",
                cmd
            );

            let mut line = String::new();
            std::io::stdin().read_line(&mut line).unwrap();
            let mut words = line.split_whitespace();

            match words.next() {
                Some("c") => break Ok(()),
                Some("s") => {
                    // Check for optional dimensions
                    let screenshot = match (words.next(), words.next(), words.next(), words.next())
                    {
                        (Some(top), Some(left), Some(width), Some(height)) => {
                            self.screenshot()?.trim(vnc::Rect {
                                top: top.parse::<u16>()?,
                                left: left.parse::<u16>()?,
                                width: width.parse::<u16>()?,
                                height: height.parse::<u16>()?,
                            })?
                        }
                        _ => self.screenshot()?,
                    };
                    let hash = screenshot.hash();
                    info!(
                        "Captured screen hash: {} ({} x {})",
                        hash, screenshot.width, screenshot.height
                    );

                    screenshot.write_png(&Path::new(&format!("screenshots/{hash}.png")))?;
                }
                Some("q") => {
                    self.debug = false;
                    break Ok(());
                }
                _ => continue,
            }
        }
    }

    pub fn boot_command(&mut self, command: Vec<Vec<Cmd>>) -> Result<(), Box<dyn Error>> {
        info!("Running bootstrap sequence");

        let progress = if self.debug {
            ProgressBar::hidden()
        } else {
            ProgressBar::new(command.iter().map(|v| v.len() as u64).sum())
        };
        progress.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}]")
                .progress_chars("=>-"),
        );
        progress.enable_steady_tick(100);

        let mut step_number = 0;
        for step in command {
            for item in step {
                progress.set_position(step_number);
                step_number += 1;

                if self.debug {
                    match &item {
                        Cmd::Wait(_) => {}
                        _ => self.handle_breakpoint(&item)?,
                    }
                }
                match item {
                    Cmd::Type(ref text) => {
                        for ch in text.chars() {
                            if ch.is_uppercase() {
                                self.vnc.send_key_event(true, 0xffe1)?;
                                self.vnc.send_key_event(true, 0x01000000 + ch as u32)?;
                                self.vnc.send_key_event(false, 0x01000000 + ch as u32)?;
                                self.vnc.send_key_event(false, 0xffe1)?;
                            } else {
                                match ch {
                                    '~' | '!' | '#' | '$' | '%' | '^' | '&' | '*' | '(' | ')'
                                    | '_' | '+' => {
                                        self.vnc.send_key_event(true, 0xffe1)?;
                                        self.vnc.send_key_event(true, 0x01000000 + ch as u32)?;
                                        self.vnc.send_key_event(false, 0x01000000 + ch as u32)?;
                                        self.vnc.send_key_event(false, 0xffe1)?;
                                    }
                                    _ => {
                                        self.vnc.send_key_event(true, 0x01000000 + ch as u32)?;
                                        self.vnc.send_key_event(false, 0x01000000 + ch as u32)?;
                                    }
                                }
                            }
                            std::thread::sleep(Duration::from_millis(200));
                        }
                    }
                    Cmd::Wait(duration) => {
                        debug!("Waiting {} seconds", &duration);
                        std::thread::sleep(Duration::from_secs(duration));
                    }
                    Cmd::WaitScreen(hash) => {
                        debug!("Waiting for screen hash to equal: {}", &hash);
                        loop {
                            std::thread::sleep(Duration::from_millis(
                                rand::thread_rng().gen_range(500..1000),
                            ));
                            if self.screenshot()?.hash() == hash {
                                // Don't continue immediately
                                std::thread::sleep(Duration::from_secs(1));
                                break;
                            }
                        }
                    }
                    Cmd::WaitScreenRect(hash, rect) => {
                        debug!("Waiting for screen hash to equal: {}", &hash);
                        loop {
                            std::thread::sleep(Duration::from_secs(1));
                            // TODO add rect
                            if self.screenshot()?.trim(rect)?.hash() == hash {
                                // Don't continue immediately
                                std::thread::sleep(Duration::from_secs(1));
                                break;
                            }
                        }
                    }
                    Cmd::Enter => {
                        self.vnc.send_key_event(true, 0xff0d)?;
                        self.vnc.send_key_event(false, 0xff0d)?;
                    }
                    Cmd::Tab => {
                        self.vnc.send_key_event(true, 0xff09)?;
                        self.vnc.send_key_event(false, 0xff09)?;
                    }
                    Cmd::Spacebar => {
                        self.vnc.send_key_event(true, 0x0020)?;
                        self.vnc.send_key_event(false, 0x0020)?;
                    }
                    Cmd::LeftSuper => {
                        self.vnc.send_key_event(true, 0xffeb)?;
                        self.vnc.send_key_event(false, 0xffeb)?;
                    }
                }
                if self.record {
                    self.screenshot()?
                        .write_png(&Path::new(&format!("screenshots/{step_number}.png")))?;
                }
            }
        }
        progress.finish();
        Ok(())
    }
}

pub mod bootcmds {

    macro_rules! enter {
        ($text:expr) => {
            vec![
                crate::vnc::Cmd::Type($text.to_string()),
                crate::vnc::Cmd::Enter,
                crate::vnc::Cmd::Wait(2),
            ]
        };
        () => {
            vec![crate::vnc::Cmd::Enter, crate::vnc::Cmd::Wait(2)]
        };
    }

    macro_rules! spacebar {
        () => {
            vec![crate::vnc::Cmd::Spacebar, crate::vnc::Cmd::Wait(2)]
        };
    }

    macro_rules! tab {
        ($text:expr) => {
            vec![
                crate::vnc::Cmd::Type($text.to_string()),
                crate::vnc::Cmd::Tab,
                crate::vnc::Cmd::Wait(2),
            ]
        };
        () => {
            vec![crate::vnc::Cmd::Tab, crate::vnc::Cmd::Wait(2)]
        };
    }

    macro_rules! wait {
        ($duration:expr) => {
            vec![crate::vnc::Cmd::Wait($duration)]
        };
    }

    macro_rules! wait_screen {
        ($hash:expr) => {
            vec![crate::vnc::Cmd::WaitScreen($hash.to_string())]
        };
    }

    macro_rules! wait_screen_rect {
        ($hash:expr, $top:expr, $left:expr, $width:expr, $height:expr) => {
            vec![crate::vnc::Cmd::WaitScreenRect(
                $hash.to_string(),
                vnc::Rect {
                    top: $top,
                    left: $left,
                    width: $width,
                    height: $height,
                },
            )]
        };
    }

    macro_rules! input {
        ($text:expr) => {
            vec![
                crate::vnc::Cmd::Type($text.to_string()),
                crate::vnc::Cmd::Wait(1),
            ]
        };
    }

    macro_rules! leftSuper {
        () => {
            vec![crate::vnc::Cmd::LeftSuper, crate::vnc::Cmd::Wait(2)]
        };
    }

    pub(crate) use enter;
    pub(crate) use input;
    pub(crate) use leftSuper;
    pub(crate) use spacebar;
    pub(crate) use tab;
    pub(crate) use wait;
    pub(crate) use wait_screen;
    pub(crate) use wait_screen_rect;
}
