use log::{debug, info};
use sha1::{Digest, Sha1};
use simple_error::bail;
use std::error::Error;
use std::fs::File;
use std::io::BufWriter;
use std::net::TcpStream;
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

    pub fn write_png(&self, output_path: &str) -> Result<(), Box<dyn Error>> {
        let ref mut w = BufWriter::new(File::create(output_path)?);

        let mut encoder = png::Encoder::new(w, self.width as u32, self.height as u32);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&self.data).unwrap();
        Ok(())
    }

    /// Compute a percentage of how similar the given screenshot is to this one.
    pub fn similarity(&self, _other: &VncScreenshot) -> f32 {
        0.0
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
    pub debug: bool,
}

impl VncConnection {
    pub fn new(host: &str, port: u16) -> Result<VncConnection, Box<dyn Error>> {
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

        debug!("Connected to VNC ({} x {})", width, height);

        // Qemu hacks
        vnc.set_encodings(&[vnc::Encoding::Raw, vnc::Encoding::DesktopSize])?;

        Ok(Self {
            width,
            height,
            vnc: vnc,
            debug: true,
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
                _ => {},
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
                    _ => bail!("VNC poll failed"),
                }
            }
        }
    }

    fn handle_breakpoint(&mut self, cmd: &Cmd) -> Result<(), Box<dyn Error>> {
        info!(
            "(breakpoint)['c' to continue, 's' to screenshot, 'q' to quit] Next command: {:?}",
            cmd
        );

        loop {
            let mut line = String::new();
            std::io::stdin().read_line(&mut line).unwrap();
            match line.chars().next().unwrap() {
                'c' => break Ok(()),
                's' => {
                    let screenshot = self.screenshot()?;
                    info!("Current screen hash: {}", screenshot.hash());
                    screenshot.write_png("/tmp/test.png")?;
                }
                'q' => panic!(),
                _ => continue,
            }
        }
    }

    pub fn boot_command(&mut self, command: Vec<Vec<Cmd>>) -> Result<(), Box<dyn Error>> {
        for step in command {
            for item in step {
                if self.debug {
                    self.handle_breakpoint(&item)?;
                }
                match item {
                    Cmd::Type(text) => {
                        for ch in text.chars() {
                            self.vnc.send_key_event(true, 0x01000000 + ch as u32)?;
                            self.vnc.send_key_event(false, 0x01000000 + ch as u32)?;
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
                            std::thread::sleep(Duration::from_secs(1));
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
                            if self.screenshot()?.hash() == hash {
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
                    Cmd::Tab => {}
                    Cmd::Spacebar => {}
                    Cmd::LeftSuper => {
                        self.vnc.send_key_event(true, 0xffeb)?;
                        self.vnc.send_key_event(false, 0xffeb)?;
                    }
                }
            }
        }
        Ok(())
    }
}

pub mod bootcmds {

    macro_rules! enter {
        ($text:expr) => {
            vec![
                crate::vnc::Cmd::Type($text.to_string()),
                crate::vnc::Cmd::Enter,
                crate::vnc::Cmd::Wait(1),
            ]
        };
        () => {
            vec![crate::vnc::Cmd::Enter, crate::vnc::Cmd::Wait(1)]
        };
    }

    macro_rules! spacebar {
        () => {
            vec![crate::vnc::Cmd::Spacebar, crate::vnc::Cmd::Wait(1)]
        };
    }

    macro_rules! tab {
        ($text:expr) => {
            vec![
                crate::vnc::Cmd::Type($text.to_string()),
                crate::vnc::Cmd::Tab,
                crate::vnc::Cmd::Wait(1),
            ]
        };
        () => {
            vec![crate::vnc::Cmd::Tab, crate::vnc::Cmd::Wait(1)]
        };
    }

    macro_rules! wait {
        ($duration:expr) => {
            vec![crate::vnc::Cmd::Wait($duration)]
        };
    }

    macro_rules! wait_screen {
        ($hash:expr) => {
            vec![crate::vnc::Cmd::WaitScreen($hash)]
        };
    }

    macro_rules! wait_screen_rect {
        ($hash:expr, $top:expr, $left:expr, $width:expr, $height:expr) => {
            vec![crate::vnc::Cmd::WaitScreenRect($hash.to_string(), vnc::Rect{top: $top, left: $left, width: $width, height: $height})]
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
            vec![crate::vnc::Cmd::LeftSuper, crate::vnc::Cmd::Wait(1)]
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
