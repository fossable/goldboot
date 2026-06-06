use std::path::PathBuf;

#[cfg(feature = "build")]
pub mod build;
pub mod deploy;
pub mod drift;
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

        /// Tag for the resulting image. Defaults to a UTC timestamp like
        /// 20260606T143022.
        #[clap(long)]
        tag: Option<String>,

        /// Image name. Required when the config file is `goldboot.ron`;
        /// inferred from the filename when it is `<name>.goldboot.ron`.
        #[clap(long)]
        name: Option<String>,

        /// Context directory containing goldboot.ron or <name>.goldboot.ron
        #[clap(index = 1)]
        path: String,
    },

    /// Manage local images
    Image {
        #[clap(subcommand)]
        command: ImageCommands,
    },

    /// Write images to storage
    Deploy {
        /// Image reference: `<host>/<name>[:<tag>]`. Tag defaults to the
        /// newest image with that name.
        #[clap(index = 1)]
        image: String,

        /// Output destination path
        #[clap(long)]
        output: String,

        /// Do not prompt for confirmation (be extremely careful with this)
        #[clap(long, num_args = 0)]
        confirm: bool,
    },

    /// Check how much a device has drifted from an image
    Drift {
        /// Image reference: `<host>/<name>[:<tag>]`. Tag defaults to the
        /// newest image with that name.
        #[clap(index = 1)]
        image: String,

        /// Device or file path to check
        #[clap(long)]
        input: String,
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

    /// Install goldboot to a boot partition.
    Install {
        /// Destination path (EFI system partition mount point)
        #[clap(long, default_value = "/boot")]
        dest: String,

        /// Optional images to include
        #[clap(long, value_enum)]
        include: Vec<String>,

        /// Never make any actual changes
        #[clap(long)]
        dryrun: bool,

        /// Write to EFI/BOOT/BOOTX64.EFI to take over default boot precedence
        #[clap(long)]
        takeover: bool,
    },

    /// Serve the goldboot LSP
    Lsp,
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum ImageCommands {
    /// List images. Defaults to the local image library; pass a registry
    /// address to list images from a remote registry instead.
    List {
        /// Optional remote registry to query (e.g. registry.example.com).
        /// When omitted, lists the local image library.
        #[clap(index = 1)]
        registry: Option<String>,

        /// HTTP Basic Auth username (if your registry's proxy requires auth)
        #[clap(short = 'u', long, env = "GOLDBOOT_REGISTRY_USERNAME")]
        username: Option<String>,

        /// HTTP Basic Auth password
        #[clap(short = 'p', long, env = "GOLDBOOT_REGISTRY_PASSWORD")]
        password: Option<String>,
    },

    /// Get detailed image info
    Info {
        /// Image reference: `<host>/<name>[:<tag>]`. Tag defaults to the
        /// newest image with that name.
        #[clap(index = 1)]
        image: String,
    },

    /// Delete local images
    Delete {
        /// One or more image references: `<host>/<name>[:<tag>]`.
        #[clap(required = true)]
        images: Vec<String>,
    },

    /// Upload a local image to a remote registry (e.g. registry.example.com/archlinux:v1)
    Push {
        /// Image reference in the form host/name[:tag]
        #[clap(index = 1)]
        reference: String,

        /// HTTP Basic Auth username (if your registry's proxy requires auth)
        #[clap(short = 'u', long, env = "GOLDBOOT_REGISTRY_USERNAME")]
        username: Option<String>,

        /// HTTP Basic Auth password
        #[clap(short = 'p', long, env = "GOLDBOOT_REGISTRY_PASSWORD")]
        password: Option<String>,
    },

    /// Download an image from a remote registry (e.g. registry.example.com/archlinux:latest)
    Pull {
        /// Image reference in the form host/name[:tag]
        #[clap(index = 1)]
        reference: String,

        /// HTTP Basic Auth username (if your registry's proxy requires auth)
        #[clap(short = 'u', long, env = "GOLDBOOT_REGISTRY_USERNAME")]
        username: Option<String>,

        /// HTTP Basic Auth password
        #[clap(short = 'p', long, env = "GOLDBOOT_REGISTRY_PASSWORD")]
        password: Option<String>,
    },
}
