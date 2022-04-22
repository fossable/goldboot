use colored::*;
use goldboot_core::*;
use log::{debug, info};
use simple_error::bail;
use std::{error::Error, fs, time::Instant};

#[rustfmt::skip]
fn print_banner() {
	println!("⬜{}⬜", "⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜");
	println!("⬜{}⬜", "　　　　　　　　⬛　　　⬛　⬛　　　　　　　　　　　⬛　".truecolor(200, 171, 55));
	println!("⬜{}⬜", "　　　　　　　　⬛　　　⬛　⬛　　　　　　　　　　　⬛⬛".truecolor(200, 171, 55));
	println!("⬜{}⬜", "⬛⬛⬛　⬛⬛⬛　⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛⬛　⬛　".truecolor(200, 171, 55));
	println!("⬜{}⬜", "⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　".truecolor(200, 171, 55));
	println!("⬜{}⬜", "⬛⬛⬛　⬛⬛⬛　⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛".truecolor(200, 171, 55));
	println!("⬜{}⬜", "　　⬛　　　　　　　　　　　　　　　　　　　　　　　　　".truecolor(200, 171, 55));
	println!("⬜{}⬜", "⬛⬛⬛　　　　　　　　　　　　　　　　　　　　　　　　　".truecolor(200, 171, 55));
	println!("⬜{}⬜", "⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜");
}
