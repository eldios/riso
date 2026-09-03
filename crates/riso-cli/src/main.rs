use std::process::ExitCode;

use clap::Parser;

mod apps;
mod backgrounds;
mod check;
mod cli;
mod config;
mod data;
mod dev;
mod gui;
mod output;
mod paths;
mod plugin;
mod restore;
mod theme;
mod tui;
mod wire;

use cli::{Cli, Command};

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("riso: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), String> {
    let output = cli.output.unwrap_or_else(output::default_output);
    match cli.command {
        Command::Theme { action } => theme::run(action, output),
        Command::Backgrounds { action } => backgrounds::run(action, output),
        Command::Plugin { action } => plugin::run(action, output),
        Command::Dev { action } => dev::run(action, output),
        Command::Config { action } => config::run(action, output),
        Command::Restore { state, path } => restore::restore(state, path, output),
        Command::Uninstall { state, yes } => restore::uninstall(state, yes, output),
        Command::CarouselData { what, current } => data::run(&what, current),
    }
}
