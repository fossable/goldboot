use log::debug;
use sha1::{Digest, Sha1};
use simple_error::bail;
use std::error::Error;
use std::net::TcpStream;
use std::time::Duration;
use vnc::client::Event;

pub struct VncScreenshot {
    pub data: Vec<u8>,
}

impl VncScreenshot {
    pub fn new(width: usize, height: usize) -> VncScreenshot {
        Self {
            data: vec![0; width * height],
        }
    }

    pub fn hash(&self) -> String {
        hex::encode(Sha1::new().chain_update(&self.data).finalize())
    }

    pub fn put_pixels(&mut self, rect: vnc::Rect, pixels: &Vec<u8>) {
        // TODO
    }
}

pub enum Cmd {
    Enter,
    Spacebar,
    Tab,
    LeftSuper,
    Type(String),
    Wait(u64),
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
        let vnc_format = vnc.format();
        debug!("Format: {:?}", vnc_format);

        debug!("Connected to VNC ({} x {})", width, height);

        // Qemu hacks
        vnc.set_encodings(&[vnc::Encoding::Zrle, vnc::Encoding::DesktopSize])
            .unwrap();

        Ok(Self {
            width: width,
            height: height,
            vnc: vnc,
            debug: true,
        })
    }

    pub fn screenshot(&mut self) -> Result<VncScreenshot, Box<dyn Error>> {
        let mut screen = VncScreenshot::new(self.width.into(), self.height.into());

        self.vnc
            .request_update(
                vnc::Rect {
                    left: 0,
                    top: 0,
                    width: self.width,
                    height: self.height,
                },
                false,
            )
            .unwrap();

        for event in self.vnc.poll_iter() {
            match event {
                Event::Disconnected(None) => bail!("Disconnected"),
                Event::Disconnected(Some(error)) => bail!("Disconnected"),
                Event::Resize(width, height) => {
                    self.width = width;
                    self.height = height;
                }
                Event::PutPixels(vnc_rect, ref pixels) => {
                    screen.put_pixels(vnc_rect, pixels);
                }
                _ => {}
            }
        }

        Ok(screen)
    }

    pub fn type_key(&mut self, text: String) -> Result<(), Box<dyn Error>> {
        let chr = 0x01000000 + text.chars().next().unwrap() as u32;
        self.vnc.send_key_event(true, chr)?;
        self.vnc.send_key_event(false, chr)?;
        Ok(())
    }

    pub fn boot_command(&mut self, command: Vec<Vec<Cmd>>) -> Result<(), Box<dyn Error>> {
        let mut line = String::new();

        for step in command {
            for item in step {
                match item {
                    Cmd::Type(text) => {
                        if self.debug {
                            println!("Waiting to type '{text}'");

                            // TEMP
                            loop {
                                println!("hash: {}", self.screenshot()?.hash());
                                std::thread::sleep(Duration::from_secs(5));
                            }

                            std::io::stdin().read_line(&mut line).unwrap();
                        }
                        self.type_key(text)?;
                    }
                    Cmd::Wait(duration) => {
                        debug!("Waiting {} seconds", &duration);
                        std::thread::sleep(Duration::from_secs(duration));
                    }
                    Cmd::Enter => {}
                    Cmd::Tab => {}
                    Cmd::Spacebar => {}
                    Cmd::LeftSuper => {}
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
}
