use std::cmp::Ordering;

use crate::data::CsvData;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortDirection {
    Ascending,
    Descending,
}

impl SortDirection {
    fn toggled(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SortColumn {
    column: usize,
    direction: SortDirection,
    numeric: bool,
}

#[derive(Debug)]
pub(crate) struct SortState {
    columns: Vec<SortColumn>,
    row_order: Vec<usize>,
}

impl SortState {
    pub(crate) fn new(row_count: usize) -> Self {
        Self {
            columns: Vec::new(),
            row_order: (0..row_count).collect(),
        }
    }

    pub(crate) fn displayed_row_index(&self, displayed_row: usize) -> Option<usize> {
        self.row_order.get(displayed_row).copied()
    }

    pub(crate) fn row_count(&self) -> usize {
        self.row_order.len()
    }

    pub(crate) fn indicator(&self, column: usize) -> Option<char> {
        self.columns
            .iter()
            .position(|sorted_column| sorted_column.column == column)
            .map(
                |position| match (position == 0, self.columns[position].direction) {
                    (true, SortDirection::Descending) => '▼',
                    (true, SortDirection::Ascending) => '▲',
                    (false, SortDirection::Descending) => '▽',
                    (false, SortDirection::Ascending) => '△',
                },
            )
    }

    #[cfg(test)]
    pub(crate) fn sorted_columns(&self) -> Vec<usize> {
        self.columns.iter().map(|column| column.column).collect()
    }

    pub(crate) fn sort_by_column(&mut self, data: &CsvData, column: usize) {
        if column >= data.column_count() {
            return;
        }

        match self
            .columns
            .iter()
            .position(|sorted_column| sorted_column.column == column)
        {
            Some(0) => {
                self.columns[0].direction = self.columns[0].direction.toggled();
            }
            Some(position) => {
                let sorted_column = self.columns.remove(position);
                self.columns.insert(0, sorted_column);
            }
            None => {
                self.columns.insert(
                    0,
                    SortColumn {
                        column,
                        direction: SortDirection::Descending,
                        numeric: is_numeric_column(data, column),
                    },
                );
            }
        }

        self.apply(data);
    }

    pub(crate) fn reset(&mut self) {
        self.columns.clear();
        self.row_order.sort_unstable();
    }

    pub(crate) fn replace_rows(&mut self, row_indices: Vec<usize>, data: &CsvData) {
        self.row_order = row_indices;
        self.apply(data);
    }

    fn apply(&mut self, data: &CsvData) {
        let columns = &self.columns;
        self.row_order.sort_by(|left_row, right_row| {
            for sorted_column in columns {
                let left = data.rows[*left_row]
                    .get(sorted_column.column)
                    .map_or("", String::as_str);
                let right = data.rows[*right_row]
                    .get(sorted_column.column)
                    .map_or("", String::as_str);
                let ordering = if sorted_column.numeric {
                    compare_numeric(left, right)
                } else {
                    left.cmp(right)
                };
                let ordering = match sorted_column.direction {
                    SortDirection::Ascending => ordering,
                    SortDirection::Descending => ordering.reverse(),
                };
                if ordering != Ordering::Equal {
                    return ordering;
                }
            }

            left_row.cmp(right_row)
        });
    }
}

fn is_numeric_column(data: &CsvData, column: usize) -> bool {
    !data.rows.is_empty()
        && data.rows.iter().all(|row| {
            row.get(column)
                .is_some_and(|value| is_numeric(value.as_str()))
        })
}

fn is_numeric(value: &str) -> bool {
    let value = value.strip_prefix('-').unwrap_or(value);
    if let Some((integer, fraction)) = value.split_once('.') {
        !fraction.is_empty()
            && integer.bytes().all(|byte| byte.is_ascii_digit())
            && fraction.bytes().all(|byte| byte.is_ascii_digit())
    } else {
        !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
    }
}

fn compare_numeric(left: &str, right: &str) -> Ordering {
    let (left_negative, left) = numeric_sign_and_magnitude(left);
    let (right_negative, right) = numeric_sign_and_magnitude(right);

    match (left_negative, right_negative) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (true, true) => compare_numeric_magnitude(left, right).reverse(),
        (false, false) => compare_numeric_magnitude(left, right),
    }
}

fn numeric_sign_and_magnitude(value: &str) -> (bool, &str) {
    let Some(magnitude) = value.strip_prefix('-') else {
        return (false, value);
    };
    (!is_zero(magnitude), magnitude)
}

fn is_zero(value: &str) -> bool {
    value.bytes().all(|byte| matches!(byte, b'0' | b'.'))
}

fn compare_numeric_magnitude(left: &str, right: &str) -> Ordering {
    let (left_integer, left_fraction) = numeric_parts(left);
    let (right_integer, right_fraction) = numeric_parts(right);
    let left_integer = normalized_integer(left_integer);
    let right_integer = normalized_integer(right_integer);

    left_integer
        .len()
        .cmp(&right_integer.len())
        .then_with(|| left_integer.cmp(right_integer))
        .then_with(|| compare_fraction(left_fraction, right_fraction))
}

fn numeric_parts(value: &str) -> (&str, &str) {
    value.split_once('.').unwrap_or((value, ""))
}

fn normalized_integer(integer: &str) -> &str {
    let integer = integer.trim_start_matches('0');
    if integer.is_empty() { "0" } else { integer }
}

fn compare_fraction(left: &str, right: &str) -> Ordering {
    let width = left.len().max(right.len());
    for index in 0..width {
        let left_digit = left.as_bytes().get(index).copied().unwrap_or(b'0');
        let right_digit = right.as_bytes().get(index).copied().unwrap_or(b'0');
        let ordering = left_digit.cmp(&right_digit);
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    Ordering::Equal
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(csv: &str) -> CsvData {
        CsvData::from_reader(csv.as_bytes(), true).unwrap()
    }

    fn displayed_column(data: &CsvData, sort: &SortState, column: usize) -> Vec<String> {
        (0..data.rows.len())
            .map(|displayed_row| {
                data.rows[sort.displayed_row_index(displayed_row).unwrap()][column].clone()
            })
            .collect()
    }

    #[test]
    fn secondary_columns_are_promoted_before_their_direction_toggles() {
        let data = data("name,number,group\na,2,x\nb,10,x\nc,1,y\nd,3,y\n");
        let mut sort = SortState::new(data.rows.len());

        sort.sort_by_column(&data, 1);
        assert_eq!(displayed_column(&data, &sort, 0), ["b", "d", "a", "c"]);
        assert_eq!(sort.columns[0].direction, SortDirection::Descending);
        assert!(sort.columns[0].numeric);
        assert_eq!(sort.indicator(1), Some('▼'));

        sort.sort_by_column(&data, 2);
        assert_eq!(
            sort.columns
                .iter()
                .map(|column| column.column)
                .collect::<Vec<_>>(),
            [2, 1]
        );
        assert_eq!(displayed_column(&data, &sort, 0), ["d", "c", "b", "a"]);
        assert_eq!(sort.indicator(2), Some('▼'));
        assert_eq!(sort.indicator(1), Some('▽'));

        sort.sort_by_column(&data, 1);
        assert_eq!(
            sort.columns
                .iter()
                .map(|column| column.column)
                .collect::<Vec<_>>(),
            [1, 2]
        );
        assert_eq!(sort.columns[0].direction, SortDirection::Descending);
        assert_eq!(displayed_column(&data, &sort, 0), ["b", "d", "a", "c"]);
        assert_eq!(sort.indicator(1), Some('▼'));
        assert_eq!(sort.indicator(2), Some('▽'));

        sort.sort_by_column(&data, 1);
        assert_eq!(sort.columns[0].direction, SortDirection::Ascending);
        assert_eq!(displayed_column(&data, &sort, 0), ["c", "a", "d", "b"]);
        assert_eq!(sort.indicator(1), Some('▲'));
        assert_eq!(sort.indicator(2), Some('▽'));

        sort.sort_by_column(&data, 2);
        assert_eq!(sort.columns[0].direction, SortDirection::Descending);
        assert_eq!(displayed_column(&data, &sort, 0), ["c", "d", "a", "b"]);
        assert_eq!(sort.indicator(2), Some('▼'));
        assert_eq!(sort.indicator(1), Some('△'));
    }

    #[test]
    fn reset_clears_the_stack_and_restores_file_order() {
        let data = data("name,number\na,2\nb,10\nc,1\n");
        let mut sort = SortState::new(data.rows.len());
        sort.sort_by_column(&data, 1);

        sort.reset();

        assert!(sort.columns.is_empty());
        assert_eq!(displayed_column(&data, &sort, 0), ["a", "b", "c"]);
    }

    #[test]
    fn numeric_detection_matches_the_requested_signed_decimal_grammar() {
        for value in [
            "0", "001", "1.2", ".5", "000.0500", "-1", "-001", "-1.2", "-.5",
        ] {
            assert!(is_numeric(value), "expected {value:?} to be numeric");
        }
        for value in [
            "", ".", "-", "-.", "1.", "-1.", "--1", "+1", "1e2", " 1", "1.2.3",
        ] {
            assert!(!is_numeric(value), "expected {value:?} to be text");
        }

        assert_eq!(compare_numeric("0002.500", "2.5"), Ordering::Equal);
        assert_eq!(compare_numeric("10", "2"), Ordering::Greater);
        assert_eq!(compare_numeric(".05", ".5"), Ordering::Less);
        assert_eq!(compare_numeric("-10", "-2"), Ordering::Less);
        assert_eq!(compare_numeric("-.5", ".05"), Ordering::Less);
        assert_eq!(compare_numeric("-0.000", "0"), Ordering::Equal);
    }

    #[test]
    fn signed_numbers_sort_numerically() {
        let data = data("value\n-2\n10\n-10\n.5\n-.5\n0\n");
        let mut sort = SortState::new(data.rows.len());

        sort.sort_by_column(&data, 0);
        assert_eq!(
            displayed_column(&data, &sort, 0),
            ["10", ".5", "0", "-.5", "-2", "-10"]
        );

        sort.sort_by_column(&data, 0);
        assert_eq!(
            displayed_column(&data, &sort, 0),
            ["-10", "-2", "-.5", "0", ".5", "10"]
        );
    }

    #[test]
    fn a_column_with_any_non_numeric_cell_sorts_as_text() {
        let data = data("value\n2\n10\nx\n");
        let mut sort = SortState::new(data.rows.len());

        sort.sort_by_column(&data, 0);

        assert!(!sort.columns[0].numeric);
        assert_eq!(displayed_column(&data, &sort, 0), ["x", "2", "10"]);
    }
}
