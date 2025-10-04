use crate::cmd::Commands;
use axum::{Router, routing::get};
use clap::Parser;
use std::{env, process::ExitCode};

pub mod api;
pub mod cmd;
pub mod extract;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct CommandLine {
    #[clap(subcommand)]
    command: Option<Commands>,
}

#[derive(Clone)]
pub struct RegistryState {}

#[tokio::main]
async fn main() {
    let command_line = CommandLine::parse();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let state = RegistryState {};

    let app = Router::new()
        .route("/image/list", get(api::image::list))
        .route("/image/info/:image_id", get(api::image::info))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
