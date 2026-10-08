use std::{fs::File, io::Read, path::Path, sync::Arc};

use ratatui::text::Span;

pub const COLUMN_GAP: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvData {
    pub header: Option<Vec<String>>,
    pub rows: Arc<Vec<Vec<String>>>,
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

        Ok(Self::from_records(header, rows))
    }

    /// Transpose the complete table, including its header, padding missing cells with empty strings.
    pub(crate) fn transpose(&self) -> Self {
        let mut records = (0..self.column_count()).map(|column| {
            self.header
                .iter()
                .chain(self.rows.iter())
                .map(|record| record.get(column).cloned().unwrap_or_default())
                .collect::<Vec<_>>()
        });
        let header = self.header.as_ref().and_then(|_| records.next());
        Self::from_records(header, records.collect())
    }

    fn from_records(header: Option<Vec<String>>, rows: Vec<Vec<String>>) -> Self {
        let column_count = header
            .iter()
            .map(Vec::len)
            .chain(rows.iter().map(Vec::len))
            .max()
            .unwrap_or(0);
        let mut widths = vec![1; column_count];

        for record in header.iter().chain(rows.iter()) {
            for (column, value) in record.iter().enumerate() {
                widths[column] = widths[column].max(Span::raw(value.as_str()).width());
            }
        }

        let (column_starts, content_width) = column_geometry(&widths);

        Self {
            header,
            rows: Arc::new(rows),
            widths,
            column_starts,
            content_width,
        }
    }

    pub fn column_count(&self) -> usize {
        self.widths.len()
    }

    pub(crate) fn resize_column(&mut self, column: usize, delta: isize) {
        let Some(width) = self.widths.get(column).copied() else {
            return;
        };
        self.set_column_width(column, width.saturating_add_signed(delta));
    }

    pub(crate) fn set_column_width(&mut self, column: usize, width: usize) {
        let Some(column_width) = self.widths.get_mut(column) else {
            return;
        };
        *column_width = width.max(1);
        (self.column_starts, self.content_width) = column_geometry(&self.widths);
    }
}

fn column_geometry(widths: &[usize]) -> (Vec<usize>, usize) {
    let mut column_starts = Vec::with_capacity(widths.len());
    let mut content_width = 0usize;
    for width in widths {
        column_starts.push(content_width);
        content_width = content_width
            .saturating_add(*width)
            .saturating_add(COLUMN_GAP);
    }
    (column_starts, content_width)
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
        assert_eq!(data.content_width, 24);
        assert!(data.rows[1].get(2).is_none());
    }

    #[test]
    fn resizing_a_column_recomputes_geometry_and_clamps_to_one_cell() {
        let mut data = CsvData::from_reader("one,two,three\na,b,c\n".as_bytes(), true).unwrap();

        data.set_column_width(1, 1);
        assert_eq!(data.widths, [3, 1, 5]);
        assert_eq!(data.column_starts, [0, 5, 8]);
        assert_eq!(data.content_width, 15);

        data.resize_column(0, -20);
        assert_eq!(data.widths, [1, 1, 5]);
        assert_eq!(data.column_starts, [0, 3, 6]);
        assert_eq!(data.content_width, 13);
    }

    #[test]
    fn no_header_keeps_the_first_record_as_data() {
        let data = CsvData::from_reader("a,b\nc,d\n".as_bytes(), false).unwrap();

        assert_eq!(data.header, None);
        assert_eq!(data.rows.as_ref(), &vec![vec!["a", "b"], vec!["c", "d"]]);
    }

    #[test]
    fn transpose_uses_the_first_column_as_the_header_and_autofits() {
        let data = CsvData::from_reader("name,age\nAda,37\nBob,42\n".as_bytes(), true).unwrap();

        let transposed = data.transpose();

        assert_eq!(
            transposed.header.as_deref(),
            Some(&["name".into(), "Ada".into(), "Bob".into()][..])
        );
        assert_eq!(transposed.rows.as_ref(), &vec![vec!["age", "37", "42"]]);
        assert_eq!(transposed.widths, [4, 3, 3]);
        assert_eq!(transposed.column_starts, [0, 6, 11]);
        assert_eq!(transposed.content_width, 16);
    }

    #[test]
    fn transpose_keeps_headerless_rectangles_headerless() {
        let data = CsvData::from_reader(
            include_bytes!("../samples/headerless.csv").as_slice(),
            false,
        )
        .unwrap();

        let transposed = data.transpose();

        assert_eq!(transposed.header, None);
        assert_eq!(
            transposed.rows.as_ref(),
            &vec![vec!["Ada", "Bob"], vec!["37", "42"], vec!["London", "Rome"]]
        );
        assert_eq!(transposed.column_count(), 2);
    }

    #[test]
    fn transpose_pads_missing_header_and_row_cells() {
        let data =
            CsvData::from_reader(include_bytes!("../samples/ragged.csv").as_slice(), true).unwrap();

        let transposed = data.transpose();

        assert_eq!(
            transposed.rows.as_ref(),
            &vec![vec!["age", "37", ""], vec!["", "extra", ""]]
        );
        assert_eq!(data.header.as_ref().unwrap().len(), 2);
        assert_eq!(data.rows[1], ["Bob"]);
    }

    #[test]
    fn transpose_handles_empty_header_only_and_single_column_tables() {
        for has_header in [false, true] {
            let empty = CsvData::from_reader("".as_bytes(), has_header).unwrap();
            assert_eq!(empty.transpose(), empty);
        }

        let header_only = CsvData::from_reader("name,age\n".as_bytes(), true)
            .unwrap()
            .transpose();
        assert_eq!(header_only.header.as_deref(), Some(&["name".into()][..]));
        assert_eq!(header_only.rows.as_ref(), &vec![vec!["age"]]);

        let single_column = CsvData::from_reader("name\nAda\nBob\n".as_bytes(), true)
            .unwrap()
            .transpose();
        assert_eq!(
            single_column.header.as_deref(),
            Some(&["name".into(), "Ada".into(), "Bob".into()][..])
        );
        assert!(single_column.rows.is_empty());

        let headerless_column = CsvData::from_reader("Ada\nBob\n".as_bytes(), false)
            .unwrap()
            .transpose();
        assert_eq!(headerless_column.header, None);
        assert_eq!(headerless_column.rows.as_ref(), &vec![vec!["Ada", "Bob"]]);
    }

    #[test]
    fn transpose_preserves_escaped_fields_and_measures_unicode() {
        let data = CsvData::from_reader("label,value\n界,\"a\tb\nc\"\n".as_bytes(), true).unwrap();

        let transposed = data.transpose();

        assert_eq!(
            transposed.header.as_deref(),
            Some(&["label".into(), "界".into()][..])
        );
        assert_eq!(transposed.rows.as_ref(), &vec![vec!["value", "a\\tb\\nc"]]);
        assert_eq!(transposed.widths, [5, 7]);
    }

    #[test]
    fn escapes_controls_and_measures_unicode_cells() {
        let data =
            CsvData::from_reader(include_bytes!("../samples/text.csv").as_slice(), true).unwrap();

        assert_eq!(data.rows[3][1], "red, green, blue");
        assert_eq!(data.rows[4][1], "She said \"hello\".");
        assert_eq!(data.rows[5][1], "first line\\nsecond line");
        assert_eq!(data.rows[6][1], "left\\tright");
        assert_eq!(Span::raw(data.rows[0][1].as_str()).width(), 2);
        assert_eq!(data.widths[1], 23);
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
