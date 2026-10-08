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

    /// Treat the first record as a header (the default).
    #[arg(long, overrides_with_all = ["header", "no_header"])]
    pub header: bool,

    /// Treat the first record as data instead of a header.
    #[arg(long, overrides_with_all = ["header", "no_header"])]
    pub no_header: bool,

    /// Keep the header visible while scrolling (the default).
    #[arg(long, overrides_with_all = ["sticky_header", "no_sticky_header"])]
    pub sticky_header: bool,

    /// Scroll the header with the data instead of keeping it visible.
    #[arg(long, overrides_with_all = ["sticky_header", "no_sticky_header"])]
    pub no_sticky_header: bool,

    /// Keep column 1 visible while scrolling horizontally.
    #[arg(long, overrides_with_all = ["sticky_leader", "no_sticky_leader"])]
    pub sticky_leader: bool,

    /// Scroll column 1 with the other columns (the default).
    #[arg(long, overrides_with_all = ["sticky_leader", "no_sticky_leader"])]
    pub no_sticky_leader: bool,

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
    fn header_and_sticky_defaults_are_preserved() {
        let cli = Cli::try_parse_from(["tuisv", "data.csv"]).unwrap();
        assert!(!cli.no_header);
        assert!(!cli.no_sticky_header);
        assert!(!cli.sticky_leader);
    }

    #[test]
    fn paired_flags_accept_repetition_and_the_last_one_wins() {
        for (positive, negative) in [
            ("--header", "--no-header"),
            ("--sticky-header", "--no-sticky-header"),
            ("--sticky-leader", "--no-sticky-leader"),
        ] {
            for first in [positive, negative] {
                for second in [positive, negative] {
                    for last in [positive, negative] {
                        let cli = Cli::try_parse_from(["tuisv", first, "data.csv", second, last])
                            .unwrap();
                        let (enabled, disabled) = match positive {
                            "--header" => (cli.header, cli.no_header),
                            "--sticky-header" => (cli.sticky_header, cli.no_sticky_header),
                            "--sticky-leader" => (cli.sticky_leader, cli.no_sticky_leader),
                            _ => unreachable!(),
                        };
                        assert_eq!(enabled, last == positive);
                        assert_eq!(disabled, last == negative);
                    }
                }
            }
        }
    }

    #[test]
    fn paired_flags_override_independently() {
        let cli = Cli::try_parse_from([
            "tuisv",
            "--no-header",
            "--no-sticky-header",
            "--sticky-leader",
            "--header",
            "--no-sticky-leader",
            "--sticky-header",
            "--sticky-leader",
            "data.tsv",
        ])
        .unwrap();
        assert!(!cli.no_header);
        assert!(!cli.no_sticky_header);
        assert!(cli.sticky_leader);
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
