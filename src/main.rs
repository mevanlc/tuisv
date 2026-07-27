mod app;
mod cli;
mod data;
mod ui;

use std::{error::Error, io, process::ExitCode};

use app::App;
use clap::Parser;
use cli::Cli;
use data::CsvData;

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
    let data = CsvData::load(&cli.csvfile, !cli.no_header).map_err(|error| {
        io::Error::other(format!("failed to read {}: {error}", cli.csvfile.display()))
    })?;
    let sticky_header = !cli.no_header && !cli.no_sticky_header;
    let mut app = App::new(data, sticky_header);
    let mut terminal = app::TerminalSession::start()?;

    terminal.run(&mut app)?;
    Ok(())
}
