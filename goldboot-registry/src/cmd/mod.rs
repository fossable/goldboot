#[derive(clap::Subcommand, Debug)]
pub enum Commands {
    /// Run the registry server
    Start {},
}
