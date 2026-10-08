use std::path::PathBuf;

use clap::{ArgGroup, Parser};

use crate::data::InputFormat;

/// View a CSV or TSV file in an interactive terminal grid.
#[derive(Debug, Parser)]
#[command(version, about, group(ArgGroup::new("format").args(["csv", "tsv", "detect"])))]
pub struct Cli {
    /// Force comma-separated input.
    #[arg(long)]
    pub csv: bool,

    /// Force tab-separated input.
    #[arg(long)]
    pub tsv: bool,

    /// Auto-detect CSV or TSV (the default).
    #[arg(long)]
    pub detect: bool,

    /// Treat the first record as data instead of a header.
    #[arg(long)]
    pub no_header: bool,

    /// Scroll the header with the data instead of keeping it visible.
    #[arg(long)]
    pub no_sticky_header: bool,

    /// CSV or TSV file to view.
    #[arg(value_name = "FILE")]
    pub file: PathBuf,
}

impl Cli {
    pub fn input_format(&self) -> InputFormat {
        if self.csv {
            InputFormat::Csv
        } else if self.tsv {
            InputFormat::Tsv
        } else {
            InputFormat::Detect
        }
    }
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
        assert_eq!(cli.file, PathBuf::from("data.csv"));
        assert_eq!(cli.input_format(), InputFormat::Detect);
    }

    #[test]
    fn requires_exactly_one_file() {
        assert!(Cli::try_parse_from(["tuisv"]).is_err());
        assert!(Cli::try_parse_from(["tuisv", "one.csv", "two.csv"]).is_err());
    }

    #[test]
    fn parses_explicit_formats_with_header_options() {
        for (flag, format) in [
            ("--csv", InputFormat::Csv),
            ("--tsv", InputFormat::Tsv),
            ("--detect", InputFormat::Detect),
        ] {
            let cli = Cli::try_parse_from(["tuisv", flag, "--no-header", "data.txt"]).unwrap();
            assert_eq!(cli.input_format(), format);
            assert!(cli.no_header);
            assert_eq!(cli.detect, flag == "--detect");
        }
    }

    #[test]
    fn format_flags_are_mutually_exclusive() {
        for (first, second) in [
            ("--csv", "--tsv"),
            ("--csv", "--detect"),
            ("--tsv", "--detect"),
        ] {
            for flags in [[first, second], [second, first]] {
                let error =
                    Cli::try_parse_from(["tuisv", flags[0], flags[1], "data.txt"]).unwrap_err();
                assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
            }
        }
    }
}
