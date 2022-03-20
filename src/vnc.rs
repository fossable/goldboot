use std::net::TcpStream;
use std::error::Error;
use vnc::client::Event;
use simple_error::bail;

pub struct VncScreenshot {
	pub data: Vec<u8>,
}

impl VncScreenshot {
	pub fn new() -> VncScreenshot {
		Self{
			data: vec![],
		}
	}
}

pub enum Cmd {
	Enter(),
	Type(String),
}

pub struct VncConnection {
	pub width: u16,
	pub height: u16,
	pub vnc: vnc::Client,
}

impl VncConnection {
	pub fn new(host: &str, port: u16) -> Result<VncConnection, Box<dyn Error>> {
		let mut vnc = vnc::Client::from_tcp_stream(TcpStream::connect((host, port))?, true, |_| {
            Some(vnc::client::AuthChoice::None)
		})?;

		let (width, height) = vnc.size();
		let vnc_format = vnc.format();

        // Qemu hacks
        vnc.set_encodings(&[vnc::Encoding::Zrle, vnc::Encoding::DesktopSize]).unwrap();

        Ok(Self {
        	width: width,
        	height: height,
        	vnc: vnc,
        })
	}

	pub fn screenshot(&mut self) -> Result<VncScreenshot, Box<dyn Error>> {

		let mut screen = VncScreenshot::new();

		self.vnc.request_update(vnc::Rect { left: 0, top: 0, width: self.width, height: self.height },false).unwrap();

		for event in self.vnc.poll_iter() {
			match event {
				Event::Disconnected(None) => bail!("Disconnected"),
				Event::Disconnected(Some(error)) => bail!("Disconnected"),
				Event::Resize(width, height) => {
					self.width = width;
					self.height = height;
				},
				Event::PutPixels(vnc_rect, ref pixels) => {

				},
				_ => {},
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

	pub fn boot_command(&self, command: Vec<String>) {

	}
}