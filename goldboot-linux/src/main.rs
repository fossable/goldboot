use clap::Parser;
use std::process::ExitCode;

pub mod gui;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct CommandLine {
    /// Run the GUI in fullscreen mode
    #[clap(long, num_args = 0)]
    fullscreen: bool,
}

fn main() -> ExitCode {
    let command_line = CommandLine::parse();
    crate::gui::load_gui(command_line.fullscreen)
}
