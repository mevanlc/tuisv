mod app;
mod cli;
mod data;
mod filter;
mod sort;
mod ui;

use std::{error::Error, io, process::ExitCode};

use app::App;
use clap::Parser;
use cli::Cli;
use data::TableData;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("tuisv: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let data = TableData::load(&cli.file, !cli.no_header, cli.input_format()).map_err(|error| {
        io::Error::other(format!("failed to read {}: {error}", cli.file.display()))
    })?;
    let sticky_header = !cli.no_header && !cli.no_sticky_header;
    let mut app = App::new(data, sticky_header, cli.sticky_leader);
    let mut terminal = app::TerminalSession::start()?;

    terminal.run(&mut app)?;
    Ok(())
}
