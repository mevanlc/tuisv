use std::path::PathBuf;

use clap::Parser;

/// View a CSV file in an interactive terminal grid.
#[derive(Debug, Parser)]
#[command(version, about)]
pub struct Cli {
    /// Treat the first CSV record as data instead of a header.
    #[arg(long)]
    pub no_header: bool,

    /// Scroll the header with the data instead of keeping it visible.
    #[arg(long)]
    pub no_sticky_header: bool,

    /// CSV file to view.
    #[arg(value_name = "CSVFILE")]
    pub csvfile: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_file_and_header_options() {
        let cli = Cli::try_parse_from(["tuisv", "--no-header", "--no-sticky-header", "data.csv"])
            .unwrap();

        assert!(cli.no_header);
        assert!(cli.no_sticky_header);
        assert_eq!(cli.csvfile, PathBuf::from("data.csv"));
    }

    #[test]
    fn requires_exactly_one_csv_file() {
        assert!(Cli::try_parse_from(["tuisv"]).is_err());
        assert!(Cli::try_parse_from(["tuisv", "one.csv", "two.csv"]).is_err());
    }
}
