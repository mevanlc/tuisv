mod app;
mod cli;
mod data;
mod filter;
mod sort;
mod ui;

use std::{error::Error, io, io::Write, process::ExitCode};

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
    if cli.help_keymap {
        return match io::stdout().lock().write_all(ui::keymap_text().as_bytes()) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
            Err(error) => Err(error.into()),
        };
    }
    let file = cli
        .file
        .as_ref()
        .expect("clap requires a file to open the viewer");
    let data = TableData::load(file, !cli.no_header, cli.input_format())
        .map_err(|error| io::Error::other(format!("failed to read {}: {error}", file.display())))?;
    let sticky_header = !cli.no_header && !cli.no_sticky_header;
    let mut app = App::new(data, sticky_header, cli.sticky_leader);
    let mut terminal = app::TerminalSession::start()?;

    terminal.run(&mut app)?;
    Ok(())
}
