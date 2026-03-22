use std::path::PathBuf;

#[cfg(feature = "build")]
pub mod build;
pub mod deploy;
pub mod image;
#[cfg(feature = "build")]
pub mod init;
pub mod install;
pub mod registry;

#[derive(clap::Subcommand, Debug, Clone)]
pub enum Commands {
    /// Build a new image
    #[cfg(feature = "build")]
    Build {
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

        #[clap(long)]
        ovmf_path: Option<PathBuf>,

        /// Delete any cached build state and start fresh
        #[clap(long, num_args = 0)]
        clean: bool,

        /// The context directory containing goldboot.ron
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
    #[cfg(feature = "build")]
    Init {
        /// New image name
        #[clap(long)]
        name: Option<String>,

        /// Base operating system(s)
        #[clap(long)]
        os: Option<Vec<String>>,

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

    /// Install goldboot to a boot partition.
    Install {
        /// Destination path (EFI system partition mount point)
        #[clap(long, default_value = "/boot")]
        dest: String,

        /// Optional images to include
        #[clap(long, value_enum)]
        include: Vec<String>,

        /// Show what would be done without making any changes
        #[clap(long)]
        dryrun: bool,
    },

    /// Serve the goldboot LSP
    Lsp,
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum RegistryCommands {
    /// Authenticate with a registry
    Login {
        /// Registry URL (e.g. registry.example.com)
        #[clap(index = 1)]
        registry: String,
    },

    /// Upload a local image to a remote registry (e.g. registry.example.com/archlinux:v1)
    Push {
        /// Image reference in the form host/name[:tag]
        #[clap(index = 1)]
        reference: String,
    },

    /// Download an image from a remote registry (e.g. registry.example.com/archlinux:latest)
    Pull {
        /// Image reference in the form host/name[:tag]
        #[clap(index = 1)]
        reference: String,
    },
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum ImageCommands {
    /// List local images
    List {},

    /// Get detailed image info
    Info { image: Option<String> },

    /// Delete local images
    Delete {
        #[clap(required = true)]
        images: Vec<String>,
    },
}
