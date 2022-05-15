use std::io::Write;
use std::net::TcpListener;
use std::error::Error;
use log::info;

/// Minimal HTTP server for serving files to virtual machines
pub struct HttpServer {
	pub port: u16,
}

impl HttpServer {

	/// Server a file to all requests.
	pub fn serve_file(data: Vec<u8>) -> Result<Self, Box<dyn Error>> {
		let port = crate::find_open_port(8000, 9000);
		info!("Starting static HTTP server on port: {}", port);

		std::thread::spawn(move || {
			let listener = TcpListener::bind(format!("0.0.0.0:{port}")).unwrap();

		    for stream in listener.incoming() {
		        let mut stream = stream.unwrap();

			    stream.write(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", data.len()).as_bytes()).unwrap();
			    stream.write(&data).unwrap();
			    stream.flush().unwrap();
		    }
		});

	    Ok(Self {
	    	port,
	    })
	}
}