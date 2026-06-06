use clap::Parser;

mod api;
mod cmd;
mod storage;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct CommandLine {
    /// Address to bind. Should usually be loopback when running behind a
    /// reverse proxy.
    #[clap(long, default_value = "0.0.0.0:3000")]
    pub bind: String,

    /// Directory where uploaded images are stored.
    #[clap(long, default_value = "/var/lib/goldboot-registry")]
    pub data_dir: std::path::PathBuf,

    /// Maximum upload size in bytes (default 32 GiB).
    #[clap(long, default_value_t = cmd::start::DEFAULT_MAX_UPLOAD)]
    pub max_upload_size: u64,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = CommandLine::parse();

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(cmd::start::run(cmd::start::StartArgs {
        bind: cli.bind,
        data_dir: cli.data_dir,
        max_upload_size: cli.max_upload_size,
    }))?;

    Ok(())
}
