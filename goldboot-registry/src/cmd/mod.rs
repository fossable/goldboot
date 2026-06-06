pub mod start;

#[derive(clap::Subcommand, Debug)]
pub enum Commands {
    /// Start the registry HTTP server
    Start(start::StartArgs),
}
