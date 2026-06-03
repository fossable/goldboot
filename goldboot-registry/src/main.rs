use clap::Parser;

mod api;
mod auth;
mod cmd;
mod config;
mod storage;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct CommandLine {
    #[clap(subcommand)]
    command: cmd::Commands,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = CommandLine::parse();

    match cli.command {
        cmd::Commands::Start { config } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(cmd::start::run(&config))?;
        }
        cmd::Commands::User { command } => {
            cmd::user::run(command)?;
        }
    }
    Ok(())
}
