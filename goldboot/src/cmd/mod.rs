pub mod init;

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

		/// The target architecture
		#[clap(long)]
		arch: Option<String>,

		/// The amount of memory the image can access
		#[clap(long)]
		memory: Option<String>,

		/// The amount of storage the image can access
		#[clap(long)]
		disk: Option<String>,

		/// Attempt to copy the configuration of the current hardware as closely
		/// as possible
		#[clap(long, takes_value = false)]
		mimic_hardware: bool,

		/// List available templates and exit
		#[clap(long, takes_value = false)]
		list_templates: bool,
	},

	// TODO: pull a GBL image from a public registry instead
	/// Create a bootable USB drive
	MakeUsb {
		/// The disk to erase and make bootable
		output: String,

		/// Do not check for confirmation
		#[clap(long, takes_value = false)]
		confirm: bool,

		/// A local image to include on the boot USB
		#[clap(long)]
		include: Vec<String>,

		/// The architecture of the image to download
		#[clap(long)]
		arch: Option<String>,
	},

	/// Manage image registries
	Registry {
		#[clap(subcommand)]
		command: RegistryCommands,
	},
}

#[derive(clap::Subcommand, Debug)]
pub enum RegistryCommands {
	/// Upload a local image to a remote registry
	Push { url: String },

	/// Download an image from a remote registry
	Pull { url: String },
}

#[derive(clap::Subcommand, Debug)]
pub enum ImageCommands {
	/// List local images
	List {},

	Info {
		image: String,
	},

	/// Run an existing image
	Run {
		image: String,
	},
}
