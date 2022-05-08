pub mod abort_wait;
pub mod select_device;
pub mod select_image;
pub mod write_image;

fn main() {
	// Configure logging
	env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

	write_image::start_ui();
}
