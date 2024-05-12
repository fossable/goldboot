use crate::foundry::{molds::ImageMold, FoundryConfigPath};

pub mod cast;
pub mod deploy;
pub mod image;
pub mod init;
pub mod liveusb;
pub mod registry;

#[derive(clap::Subcommand, Debug)]
pub enum Commands {
    /// Cast (build) a new image
    Cast {
        /// Save a screenshot to ./screenshots after each boot command for
        /// debugging
        #[clap(long, num_args = 0)]
        record: bool,

        /// Insert a breakpoint after each boot command
        #[clap(long, num_args = 0)]
        debug: bool,

        /// Read the encryption password from STDIN
        #[clap(long, num_args = 0)]
        read_password: bool,

        /// Disable virtual machine acceleration even when available
        #[clap(long, num_args = 0)]
        no_accel: bool,

        /// The optional output destination (defaults to image library)
        #[clap(long)]
        output: Option<String>,

        /// The context directory (containing a goldboot config file)
        #[clap(index = 1)]
        path: String,
        // The image will be run as a virtual machine for testing
        // #[clap(long, num_args = 0)]
        // virtual: bool
    },

    /// Manage local images
    Image {
        #[clap(subcommand)]
        command: ImageCommands,
    },

    /// Write images to storage
    Deploy {
        /// The ID or path of the image to write
        #[clap(index = 1)]
        image: String,

        /// The output destination
        #[clap(long)]
        output: String,

        /// Do not prompt for confirmation (be extremely careful with this)
        #[clap(long, num_args = 0)]
        confirm: bool,
    },

    /// Initialize the current directory as a new goldboot project
    Init {
        /// New image name
        #[clap(long)]
        name: Option<String>,

        /// Base mold(s)
        #[clap(long, value_enum)]
        mold: Vec<ImageMold>,

        #[clap(long, default_value_t, value_enum)]
        format: FoundryConfigPath,

        /// The total image size
        #[clap(long)]
        size: Option<String>,

        // #[clap(long, num_args = 0)]
        // list: bool,
        /// Attempt to copy the configuration of the current hardware as closely
        /// as possible
        #[clap(long, num_args = 0)]
        mimic_hardware: bool,
    },

    /// Manage image registries
    Registry {
        #[clap(subcommand)]
        command: RegistryCommands,
    },

    /// Create a bootable live USB
    Liveusb {
        /// Destination device path
        #[clap(long)]
        dest: String,

        /// Images to include in the live USB
        #[clap(long, value_enum)]
        include: Vec<String>,

        /// Do not prompt for confirmation (be extremely careful with this)
        #[clap(long, num_args = 0)]
        confirm: bool,
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
