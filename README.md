# tuisv

`tuisv` is a small, read-only CSV and TSV viewer for the terminal. It opens the file
in the alternate screen, sizes columns to their contents, and keeps the selected
cell visible during keyboard navigation.

## Install

Install from crates.io with Rust and Cargo:

```sh
cargo install tuisv --locked
```

Prebuilt binaries are available on the [GitHub releases page](https://github.com/mevanlc/tuisv/releases)
for Linux (x64/ARM64, static musl), macOS (x64/ARM64), Windows (x64/ARM64), and
Android ARM64. Extract the archive and put `tuisv` (or `tuisv.exe`) on your PATH.
Android builds target API 24 or later and are intended for terminal environments
such as Termux; macOS builds target macOS 11 or later.

Each release includes `SHA256SUMS`, the exact published crate, and a manifest
recording the source commit, toolchain, and asset hashes. See the
[release guide](https://github.com/mevanlc/tuisv/blob/main/RELEASING.md) for verification,
release operations, and adapting this workflow to another project.

To build a checkout locally, run `cargo install --path . --locked`. Checkout
builds report `0.0.0`; release builds get their version from the Git tag.

## Usage

```text
tuisv [OPTIONS] <FILE>

Options:
      --csv               Force comma-separated input
      --tsv               Force tab-separated input
      --detect            Auto-detect CSV or TSV (the default)
      --header            Treat the first record as a header (the default)
      --no-header         Treat the first record as data
      --sticky-header     Keep the header visible while scrolling (the default)
      --no-sticky-header  Scroll the header with the data
      --sticky-leader     Keep column 1 visible while scrolling horizontally
      --no-sticky-leader  Scroll column 1 with the other columns (the default)
  -h, --help              Print help
  -V, --version           Print version
```

For each pair (`--header` / `--no-header`, `--sticky-header` /
`--no-sticky-header`, and `--sticky-leader` / `--no-sticky-leader`), the last flag
wins. Headers and sticky headers are enabled by default; the sticky leader is
disabled by default. Without a header, the sticky-header setting has no effect.
The sticky leader keeps the first column's cells, header, and filter field fixed
while the remaining columns scroll horizontally. In a narrow terminal, the leader
is clipped to leave at least one cell for the other columns. After transposing,
the first column of the current orientation stays fixed.

Format detection uses the file's contents, regardless of its extension. It parses
logical records with quoting and embedded newlines, then applies these rules in
order:

1. CSV if every record has the first record's field count and that first record
   has more than one field.
2. TSV if every record has the first record's field count and that first record
   has more than one field.
3. CSV if the first record has more than one field and no record is longer.
4. TSV if the first record has more than one field and no record is longer.
5. Otherwise, prefer a delimiter that separates the first record; if neither does,
   prefer one that separates a later record. CSV wins ties and is the fallback for
   empty or single-column input.

Only separators outside quotes count. With `--no-header`, the first data record
still supplies the comparison width. The fallback also handles ragged tables with
rows longer than their header. All parsed columns are retained, including entirely
empty columns: `name,` defines two columns. Both formats use CSV-style double
quoting, so a field containing its separator or an embedded newline can be quoted,
and a quote inside such a field is doubled.

Commas in TSV header names and tabs in CSV header names usually resolve through
record widths. For multiple columns, rectangular parses take precedence over
parses with shorter rows, and CSV wins ties. Use `--csv` or `--tsv` to override
detection. These flags and `--detect` are mutually exclusive.

Use the arrow keys to move the selected cell. Ctrl-Shift-Left and
Ctrl-Shift-Right shrink or grow its column; header `│` handles can also be dragged
to resize columns. Truncated cells end in a gray `…`. The vertical and horizontal
mouse wheels pan the viewport without changing the selection; Shift-wheel is a
horizontal-scroll fallback.

Click a column header or press Ctrl-S to sort the selected cell's column. The
first sort is descending. Sorting the primary column again toggles its direction;
sorting a secondary column promotes it to primary without changing its direction,
and the following sort toggles it. Newly sorted columns become primary and retain
the existing columns as secondary sorts. `▼` and `▲` mark primary descending and
ascending sorts; `▽` and `△` mark their secondary counterparts. Numeric columns
containing only integers or
decimals with optional leading minus signs are ordered numerically.

Press Ctrl-F to show a filter row above the header. Each column has a one-line
`fancy-regex` field; Tab and Shift-Tab move between fields, Ctrl-A moves to the
beginning of the current filter line, and a mouse click focuses a field. Nonempty
fields are combined with AND and update automatically after a short debounce on
a background worker. Invalid expressions are shown in red and match no rows until
corrected. Enter applies an edit immediately and releases textarea focus so
ordinary viewer keys work again. Ctrl-F hides the fields and removes the filtering
while remembering their values; showing the row again reapplies them.

Press Ctrl-T to toggle between the original table and its transpose. Each toggle
uses all records in file order, clears sorting and filter values, closes the
filter row, and resets selection and scrolling to the top-left. With headers,
the first column becomes the new header and the original header becomes the
first column; `--no-header` keeps both orientations headerless. Missing cells in
ragged rows appear as empty strings in the transpose. Toggling back restores the
original rows exactly, and each orientation remembers its column widths.

Press Ctrl-R to clear all sorting and filter values, restore file order, and
cancel an active filter in the current orientation. Only Ctrl-T changes
orientation. Press `?` outside an active filter textarea for the
scrollable keymap. Escape acts as Back: it closes help, then the filter row, then
resets sorting, and finally exits. Press `q` or `Q` to exit when neither a filter
textarea nor the help dialog has focus; Ctrl-C exits at any time.

The file is read fully into memory as UTF-8 CSV or TSV. Embedded control characters
are shown as escapes so each record occupies one terminal row.

## Samples

The [`samples/`](samples/) directory includes small files for trying the viewer:

- [`people.csv`](samples/people.csv): headers, sorting, filtering, and transpose.
- [`numbers.csv`](samples/numbers.csv): signed integers, decimals, and large values
  for numeric sorting.
- [`ragged.csv`](samples/ragged.csv): short records and a cell beyond the header.
- [`headerless.csv`](samples/headerless.csv): data without a header; use `--no-header`.
- [`text.csv`](samples/text.csv): Unicode, quoted commas and quotes, embedded
  newlines, and a tab.
- [`people.tsv`](samples/people.tsv): a tab-separated version of the people table.
- [`text.tsv`](samples/text.tsv): commas in a header name and values, Unicode,
  quotes, embedded newlines, and a quoted tab.

```sh
cargo run -- samples/people.csv
cargo run -- samples/people.tsv
cargo run -- --tsv samples/text.tsv
cargo run -- --no-header samples/headerless.csv
```

## License

[MIT](LICENSE).
