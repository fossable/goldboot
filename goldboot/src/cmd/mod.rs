pub mod build;
pub mod image;
pub mod init;
pub mod registry;
pub mod write;

#[derive(clap::Subcommand, Debug)]
pub enum Commands {
	/// Build a new image
	Build {
		/// Save a screenshot to ./screenshots after each boot command for
		/// debugging
		#[clap(long, takes_value = false)]
		record: bool,

		/// Insert a breakpoint after each boot command
		#[clap(long, takes_value = false)]
		debug: bool,

		/// Read the encryption password from STDIN
		#[clap(long, takes_value = false)]
		read_password: bool,

		/// The optional output destination (defaults to image library)
		#[clap(long)]
		output: Option<String>,

		/// The config file path (default: ./goldboot.json)
		#[clap(long)]
		config: Option<String>,
	},

	/// Manage local images
	Image {
		#[clap(subcommand)]
		command: ImageCommands,
	},

	/// Write images to storage
	Write {
		/// The ID of the image to write
		#[clap(long)]
		image: String,

		/// The output destination
		#[clap(long)]
		output: String,

		/// Do not prompt for confirmation (be extremely careful with this)
		#[clap(long, takes_value = false)]
		confirm: bool,
	},

	/// Initialize the current directory
	Init {
		/// The image name
		#[clap(long)]
		name: Option<String>,

		/// A base template (which can be found with --list-templates)
		#[clap(long)]
		template: Vec<String>,

		/// Attempt to copy the configuration of the current hardware as closely
		/// as possible
		#[clap(long, takes_value = false)]
		mimic_hardware: bool,
	},

	/// Manage image registries
	Registry {
		#[clap(subcommand)]
		command: RegistryCommands,
	},
}

#[derive(clap::Subcommand, Debug)]
pub enum RegistryCommands {
	/// Enter a token for a registry
	Login {},

	/// Upload a local image to a remote registry
	Push { url: String },

	/// Download an image from a remote registry
	Pull { url: String },
}

#[derive(clap::Subcommand, Debug)]
pub enum ImageCommands {
	/// List local images
	List {},

	/// Get detailed image info
	Info { image: Option<String> },
}
