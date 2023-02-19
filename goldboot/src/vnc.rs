use log::{debug, info, trace};
use rand::Rng;
use sha1::{Digest, Sha1};
use simple_error::bail;
use std::{error::Error, fs::File, io::BufWriter, net::TcpStream, path::Path, time::Duration};
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
        // Validate request
        if rect.left + rect.width > self.width || rect.top + rect.height > self.height {
            bail!(
                "Cannot trim ({} x {}) to {:?}",
                self.width,
                self.height,
                rect
            );
        }
        trace!(
            "Trimming screenshot ({} x {}) to {:?}",
            self.width,
            self.height,
            rect
        );

        let w = rect.width as usize;
        let h = rect.height as usize;
        let t = rect.top as usize;
        let l = rect.left as usize;
        let mut data = vec![0u8; w * h];

        for y in 0..rect.height as usize {
            let dst = y * w;
            let src = (y + t) * self.width as usize + l;
            data[dst..dst + w].copy_from_slice(&self.data[src..src + w]);
        }

        Ok(VncScreenshot {
            data,
            width: rect.width,
            height: rect.height,
        })
    }
}

#[derive(Debug, Clone)]
pub enum VncCmd {
    Enter,
    Spacebar,
    Tab,
    Escape,

    /// Input the left super button.
    LeftSuper,

    /// Input the given text characters with a half-second delay between each.
    Type(String),

    /// Wait the given amount of seconds.
    Wait(u64),

    /// Wait for the screen to match the given hash.
    WaitScreen(String),

    /// Wait for a section of the screen to match the given hash.
    WaitScreenRect(String, u16, u16, u16, u16),
}

/// Represents a VNC session to a running VM.
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

    fn handle_breakpoint(&mut self, cmd: &VncCmd) -> Result<(), Box<dyn Error>> {
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

    pub fn boot_command(&mut self, command: Vec<Vec<VncCmd>>) -> Result<(), Box<dyn Error>> {
        info!("Running bootstrap sequence");

        let mut step_number = 0;
        for step in command {
            for item in step {
                step_number += 1;

                if self.debug {
                    match &item {
                        VncCmd::Wait(_) => {}
                        _ => self.handle_breakpoint(&item)?,
                    }
                }
                match item {
                    VncCmd::Type(ref text) => {
                        for ch in text.chars() {
                            if ch == '\n' {
                                self.vnc.send_key_event(true, 0xff0d)?;
                                self.vnc.send_key_event(false, 0xff0d)?;
                            } else if ch.is_uppercase() {
                                self.vnc.send_key_event(true, 0xffe1)?;
                                self.vnc.send_key_event(true, 0x01000000 + ch as u32)?;
                                self.vnc.send_key_event(false, 0x01000000 + ch as u32)?;
                                self.vnc.send_key_event(false, 0xffe1)?;
                            } else {
                                match ch {
                                    '~' | '!' | '#' | '$' | '%' | '^' | '&' | '*' | '(' | ')'
                                    | '_' | '+' | '{' | '}' | '|' | ':' | '"' | '<' | '>' | '?' => {
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
                            std::thread::sleep(Duration::from_millis(100));
                        }
                    }
                    VncCmd::Wait(duration) => {
                        debug!("Waiting {} seconds", &duration);
                        std::thread::sleep(Duration::from_secs(duration));
                    }
                    VncCmd::WaitScreen(hash) => {
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
                    VncCmd::WaitScreenRect(hash, top, left, width, height) => {
                        debug!("Waiting for screen hash to equal: {}", &hash);
                        loop {
                            std::thread::sleep(Duration::from_secs(1));
                            match self.screenshot()?.trim(vnc::Rect {
                                top,
                                left,
                                width,
                                height,
                            }) {
                                Ok(screenshot) => {
                                    if screenshot.hash() == hash {
                                        // Wait a few before continuing
                                        std::thread::sleep(Duration::from_secs(1));
                                        break;
                                    }
                                }
                                // If the trim failed, the screen may not be the right size yet
                                Err(_) => continue,
                            }
                        }
                    }
                    VncCmd::Enter => {
                        self.vnc.send_key_event(true, 0xff0d)?;
                        self.vnc.send_key_event(false, 0xff0d)?;
                    }
                    VncCmd::Tab => {
                        self.vnc.send_key_event(true, 0xff09)?;
                        self.vnc.send_key_event(false, 0xff09)?;
                    }
                    VncCmd::Spacebar => {
                        self.vnc.send_key_event(true, 0x0020)?;
                        self.vnc.send_key_event(false, 0x0020)?;
                    }
                    VncCmd::LeftSuper => {
                        self.vnc.send_key_event(true, 0xffeb)?;
                        self.vnc.send_key_event(false, 0xffeb)?;
                    }
                    VncCmd::Escape => {
                        self.vnc.send_key_event(true, 0xff1b)?;
                        self.vnc.send_key_event(false, 0xff1b)?;
                    }
                }
                if self.record {
                    self.screenshot()?
                        .write_png(&Path::new(&format!("screenshots/{step_number}.png")))?;
                }
            }
        }
        Ok(())
    }
}

pub mod bootcmds {

    #[macro_export]
    macro_rules! enter {
        ($text:expr) => {
            vec![
                crate::vnc::VncCmd::Type($text.to_string()),
                crate::vnc::VncCmd::Enter,
                crate::vnc::VncCmd::Wait(2),
            ]
        };
        () => {
            vec![crate::vnc::VncCmd::Enter, crate::vnc::VncCmd::Wait(2)]
        };
    }

    #[macro_export]
    macro_rules! spacebar {
        () => {
            vec![crate::vnc::VncCmd::Spacebar, crate::vnc::VncCmd::Wait(2)]
        };
    }

    #[macro_export]
    macro_rules! escape {
        () => {
            vec![crate::vnc::VncCmd::Escape, crate::vnc::VncCmd::Wait(2)]
        };
    }

    #[macro_export]
    macro_rules! tab {
        ($text:expr) => {
            vec![
                crate::vnc::VncCmd::Type($text.to_string()),
                crate::vnc::VncCmd::Tab,
                crate::vnc::VncCmd::Wait(2),
            ]
        };
        () => {
            vec![crate::vnc::VncCmd::Tab, crate::vnc::VncCmd::Wait(2)]
        };
    }

    #[macro_export]
    macro_rules! wait {
        ($duration:expr) => {
            vec![crate::vnc::VncCmd::Wait($duration)]
        };
    }

    #[macro_export]
    macro_rules! wait_screen {
        ($hash:expr) => {
            vec![crate::vnc::VncCmd::WaitScreen($hash.to_string())]
        };
    }

    #[macro_export]
    macro_rules! wait_screen_rect {
        ($hash:expr, $top:expr, $left:expr, $width:expr, $height:expr) => {
            vec![crate::vnc::VncCmd::WaitScreenRect(
                $hash.to_string(),
                $top,
                $left,
                $width,
                $height,
            )]
        };
    }

    #[macro_export]
    macro_rules! input {
        ($text:expr) => {
            vec![
                crate::vnc::VncCmd::Type($text.to_string()),
                crate::vnc::VncCmd::Wait(1),
            ]
        };
    }

    #[macro_export]
    macro_rules! leftSuper {
        () => {
            vec![crate::vnc::VncCmd::LeftSuper, crate::vnc::VncCmd::Wait(2)]
        };
    }
}
