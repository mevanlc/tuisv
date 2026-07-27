use std::{fs::File, io::Read, path::Path};

use unicode_width::UnicodeWidthStr;

pub const COLUMN_GAP: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvData {
    pub header: Option<Vec<String>>,
    pub rows: Vec<Vec<String>>,
    pub widths: Vec<usize>,
    pub column_starts: Vec<usize>,
    pub content_width: usize,
}

impl CsvData {
    pub fn load(path: &Path, has_header: bool) -> Result<Self, csv::Error> {
        Self::from_reader(File::open(path)?, has_header)
    }

    pub fn from_reader(reader: impl Read, has_header: bool) -> Result<Self, csv::Error> {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(has_header)
            .flexible(true)
            .from_reader(reader);

        let header = if has_header {
            let record = reader.headers()?.clone();
            (!record.is_empty()).then(|| record.iter().map(display_field).collect())
        } else {
            None
        };

        let rows = reader
            .records()
            .map(|record| record.map(|record| record.iter().map(display_field).collect::<Vec<_>>()))
            .collect::<Result<Vec<_>, _>>()?;

        let column_count = header
            .iter()
            .map(Vec::len)
            .chain(rows.iter().map(Vec::len))
            .max()
            .unwrap_or(0);
        let mut widths = vec![1; column_count];

        for record in header.iter().chain(rows.iter()) {
            for (column, value) in record.iter().enumerate() {
                widths[column] = widths[column].max(UnicodeWidthStr::width(value.as_str()));
            }
        }

        let mut column_starts = Vec::with_capacity(column_count);
        let mut content_width = 0usize;
        for width in &widths {
            column_starts.push(content_width);
            content_width = content_width
                .saturating_add(*width)
                .saturating_add(COLUMN_GAP);
        }
        if column_count > 0 {
            content_width = content_width.saturating_sub(COLUMN_GAP);
        }

        Ok(Self {
            header,
            rows,
            widths,
            column_starts,
            content_width,
        })
    }

    pub fn column_count(&self) -> usize {
        self.widths.len()
    }
}

fn display_field(field: &str) -> String {
    let mut displayed = String::with_capacity(field.len());
    for character in field.chars() {
        if character.is_control() {
            displayed.extend(character.escape_default());
        } else {
            displayed.push(character);
        }
    }
    displayed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_header_and_autofits_ragged_rows() {
        let data = CsvData::from_reader(
            "name,city\nAda,London,extra\nGrace,New York\n".as_bytes(),
            true,
        )
        .unwrap();

        assert_eq!(
            data.header.as_deref(),
            Some(&["name".into(), "city".into()][..])
        );
        assert_eq!(data.rows.len(), 2);
        assert_eq!(data.column_count(), 3);
        assert_eq!(data.widths, vec![5, 8, 5]);
        assert_eq!(data.column_starts, vec![0, 7, 17]);
        assert_eq!(data.content_width, 22);
        assert!(data.rows[1].get(2).is_none());
    }

    #[test]
    fn no_header_keeps_the_first_record_as_data() {
        let data = CsvData::from_reader("a,b\nc,d\n".as_bytes(), false).unwrap();

        assert_eq!(data.header, None);
        assert_eq!(data.rows, vec![vec!["a", "b"], vec!["c", "d"]]);
    }

    #[test]
    fn escapes_controls_and_measures_unicode_cells() {
        let data = CsvData::from_reader(
            "label,value\nwide,界\ncontrol,\"a\tb\nc\"\n".as_bytes(),
            true,
        )
        .unwrap();

        assert_eq!(data.rows[1][1], "a\\tb\\nc");
        assert_eq!(UnicodeWidthStr::width(data.rows[0][1].as_str()), 2);
        assert_eq!(data.widths[1], 7);
    }

    #[test]
    fn empty_input_has_no_header_or_selection_columns() {
        let data = CsvData::from_reader("".as_bytes(), true).unwrap();

        assert_eq!(data.header, None);
        assert!(data.rows.is_empty());
        assert!(data.widths.is_empty());
        assert_eq!(data.content_width, 0);
    }

    #[test]
    fn invalid_utf8_is_an_error() {
        let result = CsvData::from_reader(&b"name\n\xff\n"[..], true);
        assert!(result.is_err());
    }
}
