use std::{fs::File, io::Read, path::Path, sync::Arc};

use ratatui::text::Span;

pub const COLUMN_GAP: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputFormat {
    Detect,
    Csv,
    Tsv,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableData {
    pub header: Option<Vec<String>>,
    pub rows: Arc<Vec<Vec<String>>>,
    pub widths: Vec<usize>,
    pub column_starts: Vec<usize>,
    pub content_width: usize,
}

impl TableData {
    pub fn load(path: &Path, has_header: bool, format: InputFormat) -> Result<Self, csv::Error> {
        let file = File::open(path)?;
        match format {
            InputFormat::Detect => Self::from_reader(file, has_header),
            InputFormat::Csv => Self::from_delimited_reader(file, has_header, b','),
            InputFormat::Tsv => Self::from_delimited_reader(file, has_header, b'\t'),
        }
    }

    /// Read CSV or TSV using the default content-based detection rules.
    pub fn from_reader(mut reader: impl Read, has_header: bool) -> Result<Self, csv::Error> {
        let mut input = Vec::new();
        reader.read_to_end(&mut input)?;
        let delimiter = detect_delimiter(&input)?;
        Self::from_delimited_reader(input.as_slice(), has_header, delimiter)
    }

    fn from_delimited_reader(
        reader: impl Read,
        has_header: bool,
        delimiter: u8,
    ) -> Result<Self, csv::Error> {
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(delimiter)
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

#[derive(Debug)]
struct RecordShape {
    first_columns: usize,
    max_columns: usize,
    rectangular: bool,
}

impl RecordShape {
    fn read(input: &[u8], delimiter: u8) -> Result<Self, csv::Error> {
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(delimiter)
            .has_headers(false)
            .flexible(true)
            .from_reader(input);
        let mut record = csv::StringRecord::new();
        let first_columns = if reader.read_record(&mut record)? {
            record.len()
        } else {
            0
        };
        let mut shape = Self {
            first_columns,
            max_columns: first_columns,
            rectangular: true,
        };
        while reader.read_record(&mut record)? {
            shape.max_columns = shape.max_columns.max(record.len());
            shape.rectangular &= record.len() == first_columns;
        }
        Ok(shape)
    }

    fn has_separated_header(&self) -> bool {
        // Multiple parsed fields mean there was a separator outside quotes.
        self.first_columns > 1
    }

    fn fits_header(&self) -> bool {
        self.has_separated_header() && self.max_columns <= self.first_columns
    }
}

fn detect_delimiter(input: &[u8]) -> Result<u8, csv::Error> {
    let csv = RecordShape::read(input, b',');
    if csv
        .as_ref()
        .is_ok_and(|shape| shape.rectangular && shape.has_separated_header())
    {
        return Ok(b',');
    }

    let tsv = RecordShape::read(input, b'\t');
    if tsv
        .as_ref()
        .is_ok_and(|shape| shape.rectangular && shape.has_separated_header())
    {
        return Ok(b'\t');
    }
    if csv.as_ref().is_ok_and(|shape| shape.fits_header()) {
        return Ok(b',');
    }
    if tsv.as_ref().is_ok_and(|shape| shape.fits_header()) {
        return Ok(b'\t');
    }

    // Ragged records can fail every rule. Prefer first-record evidence, then
    // separators in later records; retain all fields rather than hiding columns.
    match (csv, tsv) {
        (Ok(csv), Ok(tsv)) => {
            if csv.has_separated_header() {
                Ok(b',')
            } else if tsv.has_separated_header() {
                Ok(b'\t')
            } else if csv.max_columns > 1 {
                Ok(b',')
            } else if tsv.max_columns > 1 {
                Ok(b'\t')
            } else {
                Ok(b',')
            }
        }
        (Ok(_), Err(_)) => Ok(b','),
        (Err(_), Ok(_)) => Ok(b'\t'),
        (Err(error), Err(_)) => Err(error),
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
    fn detection_applies_the_four_rules_in_order() {
        for (input, delimiter) in [
            // Rule 1: CSV wins even when TSV is also rectangular.
            ("a,b\tc\n1,2\t3\n", b','),
            // Rule 2: rectangular TSV wins over shorter CSV rows (rule 3).
            ("last, first\tage\nAda\t37\nBob\t42\n", b'\t'),
            // Rule 3: shorter rows fit the CSV header.
            ("a,b,c\n1,2\n3\n", b','),
            // Rule 3 wins over rule 4 when both have shorter rows.
            ("a,b,c\td\te\n1,2\t3\n4\n", b','),
            // Rule 4: shorter rows fit the TSV header.
            ("a\tb\tc\n1\t2\n3\n", b'\t'),
        ] {
            assert_eq!(
                detect_delimiter(input.as_bytes()).unwrap(),
                delimiter,
                "{input:?}"
            );
        }
    }

    #[test]
    fn detects_tsv_with_commas_in_header_names() {
        let input = include_bytes!("../samples/text.tsv").as_slice();
        let data = TableData::from_reader(input, true).unwrap();

        assert_eq!(detect_delimiter(input).unwrap(), b'\t');
        assert_eq!(
            data.header.as_deref(),
            Some(&["kind".into(), "value, text".into(), "note".into()][..])
        );
        assert_eq!(
            data.rows[1],
            ["comma", "red, green, blue", "Literal commas"]
        );
        assert_eq!(data.rows[2][1], "She said \"hello\".");
        assert_eq!(data.rows[3][1], "first line\\nsecond line");
        assert_eq!(data.rows[4][1], "left\\tright");
        assert_eq!(data.widths[1], 23);
    }

    #[test]
    fn detects_csv_with_tabs_in_header_names() {
        let input = "name,city\tregion\nAda,London\nBob,Rome\tItaly\n";
        let data = TableData::from_reader(input.as_bytes(), true).unwrap();

        assert_eq!(detect_delimiter(input.as_bytes()).unwrap(), b',');
        assert_eq!(
            data.header.as_deref(),
            Some(&["name".into(), "city\\tregion".into()][..])
        );
        assert_eq!(data.rows[1], ["Bob", "Rome\\tItaly"]);
    }

    #[test]
    fn detection_uses_logical_records_and_quoted_delimiters() {
        for (input, delimiter, header) in [
            (
                "\"name, alias\"\tage\nAda\t37\n",
                b'\t',
                vec!["name, alias", "age"],
            ),
            (
                "\"name\talias\",age\nAda,37\n",
                b',',
                vec!["name\\talias", "age"],
            ),
            (
                "\"name\nlabel\",age\n\"Ada\nLovelace\",37\n",
                b',',
                vec!["name\\nlabel", "age"],
            ),
            (
                "\"name\nlabel\"\tage\n\"Ada\nLovelace\"\t37\n",
                b'\t',
                vec!["name\\nlabel", "age"],
            ),
        ] {
            assert_eq!(
                detect_delimiter(input.as_bytes()).unwrap(),
                delimiter,
                "{input:?}"
            );
            let data = TableData::from_reader(input.as_bytes(), true).unwrap();
            assert_eq!(data.header.unwrap(), header);
            assert_eq!(data.rows.len(), 1);
            assert_eq!(data.rows[0].len(), 2);
        }

        let single =
            TableData::from_reader(b"\"name, alias\"\n\"Ada, Lovelace\"\n".as_slice(), true)
                .unwrap();
        assert_eq!(single.column_count(), 1);
        assert_eq!(single.header.as_deref(), Some(&["name, alias".into()][..]));
        assert_eq!(single.rows[0], ["Ada, Lovelace"]);
    }

    #[test]
    fn fallback_keeps_ragged_tables_and_prefers_first_record_evidence() {
        for (input, delimiter, columns) in [
            ("name,age\nAda,37,extra\nBob\n", b',', 3),
            ("name\tage\nAda\t37\textra\nBob\n", b'\t', 3),
            // A comma appears only later, but the TSV header provides stronger evidence.
            ("name\tage\nAda\t37\textra,first,second\nBob\n", b'\t', 3),
            // Single-field headers can still have separated data records.
            ("name\nAda,\nBob\n", b',', 2),
            ("name\nAda\t\nBob\n", b'\t', 2),
            // CSV wins when both parses have only later-record separators.
            ("name\nAda,37\textra\nBob\n", b',', 2),
        ] {
            assert_eq!(
                detect_delimiter(input.as_bytes()).unwrap(),
                delimiter,
                "{input:?}"
            );
            let data = TableData::from_reader(input.as_bytes(), true).unwrap();
            assert_eq!(data.column_count(), columns);
            assert_eq!(data.rows[1], ["Bob"]);
        }
    }

    #[test]
    fn detection_preserves_explicitly_empty_columns() {
        for input in [
            "name,\nAda,\nBob,\n",
            "name\t\nAda\t\nBob\t\n",
            "name,\nAda\nBob,\n",
            "name\t\nAda\nBob\t\n",
        ] {
            let data = TableData::from_reader(input.as_bytes(), true).unwrap();
            assert_eq!(data.column_count(), 2, "{input:?}");
            assert_eq!(
                data.header.as_deref(),
                Some(&["name".into(), "".into()][..])
            );
            assert_eq!(data.rows[1], ["Bob", ""]);
            assert_eq!(data.widths[1], 1);
        }
    }

    #[test]
    fn detects_empty_single_column_and_header_only_input() {
        for (input, columns, delimiter) in [
            ("", 0, b','),
            ("\n\r\n", 0, b','),
            ("name\nAda\nBob\n", 1, b','),
            ("\"\"\n\"\"\n", 1, b','),
            ("name,age\n", 2, b','),
            ("name\tage\n", 2, b'\t'),
            (",\n,\n", 2, b','),
            ("\t\n\t\n", 2, b'\t'),
        ] {
            assert_eq!(
                detect_delimiter(input.as_bytes()).unwrap(),
                delimiter,
                "{input:?}"
            );
            assert_eq!(
                TableData::from_reader(input.as_bytes(), true)
                    .unwrap()
                    .column_count(),
                columns
            );
        }
    }

    #[test]
    fn detection_compares_the_first_record_without_a_header() {
        let data =
            TableData::from_reader(include_bytes!("../samples/people.tsv").as_slice(), false)
                .unwrap();
        assert_eq!(data.header, None);
        assert_eq!(data.rows[0], ["name", "age", "city"]);
        assert_eq!(data.rows[1], ["Ada", "37", "London"]);
        assert_eq!(data.column_count(), 3);
    }

    #[test]
    fn detection_handles_bom_blank_lines_and_crlf() {
        for (input, delimiter) in [
            ("\u{feff}\r\nname,age\r\nAda,37\r\n\r\nBob,42\r\n", b','),
            ("\u{feff}\r\nname\tage\r\nAda\t37\r\n\r\nBob\t42\r\n", b'\t'),
        ] {
            assert_eq!(detect_delimiter(input.as_bytes()).unwrap(), delimiter);
            let data = TableData::from_reader(input.as_bytes(), true).unwrap();
            assert_eq!(
                data.header.as_deref(),
                Some(&["name".into(), "age".into()][..])
            );
            assert_eq!(
                data.rows.as_ref(),
                &vec![vec!["Ada", "37"], vec!["Bob", "42"]]
            );
        }
    }

    #[test]
    fn explicit_formats_bypass_detection_and_ignore_file_extensions() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("samples/people.tsv");
        let detected = TableData::load(&path, true, InputFormat::Detect).unwrap();
        let tsv = TableData::load(&path, true, InputFormat::Tsv).unwrap();
        let csv = TableData::load(&path, true, InputFormat::Csv).unwrap();
        assert_eq!(detected, tsv);
        assert_eq!(tsv.column_count(), 3);
        assert_eq!(csv.column_count(), 1);
        assert_eq!(csv.rows[0], ["Ada\\t37\\tLondon"]);

        let path = path.with_file_name("people.csv");
        let detected = TableData::load(&path, true, InputFormat::Detect).unwrap();
        let csv = TableData::load(&path, true, InputFormat::Csv).unwrap();
        let tsv = TableData::load(&path, true, InputFormat::Tsv).unwrap();
        assert_eq!(detected, csv);
        assert_eq!(csv.column_count(), 3);
        assert_eq!(tsv.column_count(), 1);
        assert_eq!(tsv.rows[0], ["Ada,37,London"]);
    }

    #[test]
    fn reads_header_and_autofits_ragged_rows() {
        let data = TableData::from_reader(
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
        let mut data = TableData::from_reader("one,two,three\na,b,c\n".as_bytes(), true).unwrap();

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
        let data = TableData::from_reader("a,b\nc,d\n".as_bytes(), false).unwrap();

        assert_eq!(data.header, None);
        assert_eq!(data.rows.as_ref(), &vec![vec!["a", "b"], vec!["c", "d"]]);
    }

    #[test]
    fn transpose_uses_the_first_column_as_the_header_and_autofits() {
        let data = TableData::from_reader("name,age\nAda,37\nBob,42\n".as_bytes(), true).unwrap();

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
        let data = TableData::from_reader(
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
        let data = TableData::from_reader(include_bytes!("../samples/ragged.csv").as_slice(), true)
            .unwrap();

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
            let empty = TableData::from_reader("".as_bytes(), has_header).unwrap();
            assert_eq!(empty.transpose(), empty);
        }

        let header_only = TableData::from_reader("name,age\n".as_bytes(), true)
            .unwrap()
            .transpose();
        assert_eq!(header_only.header.as_deref(), Some(&["name".into()][..]));
        assert_eq!(header_only.rows.as_ref(), &vec![vec!["age"]]);

        let single_column = TableData::from_reader("name\nAda\nBob\n".as_bytes(), true)
            .unwrap()
            .transpose();
        assert_eq!(
            single_column.header.as_deref(),
            Some(&["name".into(), "Ada".into(), "Bob".into()][..])
        );
        assert!(single_column.rows.is_empty());

        let headerless_column = TableData::from_reader("Ada\nBob\n".as_bytes(), false)
            .unwrap()
            .transpose();
        assert_eq!(headerless_column.header, None);
        assert_eq!(headerless_column.rows.as_ref(), &vec![vec!["Ada", "Bob"]]);
    }

    #[test]
    fn transpose_preserves_escaped_fields_and_measures_unicode() {
        let data =
            TableData::from_reader("label,value\n界,\"a\tb\nc\"\n".as_bytes(), true).unwrap();

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
            TableData::from_reader(include_bytes!("../samples/text.csv").as_slice(), true).unwrap();

        assert_eq!(data.rows[3][1], "red, green, blue");
        assert_eq!(data.rows[4][1], "She said \"hello\".");
        assert_eq!(data.rows[5][1], "first line\\nsecond line");
        assert_eq!(data.rows[6][1], "left\\tright");
        assert_eq!(Span::raw(data.rows[0][1].as_str()).width(), 2);
        assert_eq!(data.widths[1], 23);
    }

    #[test]
    fn empty_input_has_no_header_or_selection_columns() {
        let data = TableData::from_reader("".as_bytes(), true).unwrap();

        assert_eq!(data.header, None);
        assert!(data.rows.is_empty());
        assert!(data.widths.is_empty());
        assert_eq!(data.content_width, 0);
    }

    #[test]
    fn invalid_utf8_is_an_error() {
        let result = TableData::from_reader(&b"name\n\xff\n"[..], true);
        assert!(result.is_err());
    }
}
