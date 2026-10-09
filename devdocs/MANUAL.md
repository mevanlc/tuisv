# tuisv manual

`tuisv` is a read-only CSV and TSV viewer. It opens the file in the alternate
screen, sizes columns to their contents, and keeps the selected cell visible
during keyboard navigation. Sorting, filtering, transposing, resizing, and
reordering change only the view; the source file is never rewritten.

See the [README](../README.md) for installation, command-line options, and the
keymap. This manual explains how those controls behave.

## Input and format detection

The file is read fully into memory as UTF-8 CSV or TSV. Embedded control characters
are shown as escapes so each record occupies one terminal row.

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
detection. These flags and `--detect` are mutually exclusive; detection is the
default.

## Headers and sticky columns

For each pair (`--header` / `--no-header`, `--sticky-header` /
`--no-sticky-header`, and `--sticky-leader` / `--no-sticky-leader`), the last flag
wins. Headers and sticky headers are enabled by default; the sticky leader is
disabled by default. Without a header, the sticky-header setting has no effect.

The sticky leader keeps the first column's cells, header, and filter field fixed
while the remaining columns scroll horizontally. In a narrow terminal, the leader
is clipped to leave at least one cell for the other columns. After reordering or
transposing, the first displayed column of the current orientation stays fixed.

## Selection and scrolling

Use the arrow keys to move the selected cell, or left-click a data cell to select
it and release filter focus. Cell padding and empty cells are selectable; column
gaps and space outside the table are ignored. Clicking keeps the viewport in place
and leaves filter values and ongoing filtering active.

The vertical and horizontal mouse wheels pan the viewport without changing the
selection; Shift-wheel is a horizontal-scroll fallback.

## Column order and widths

Shift-Left and Shift-Right swap the selected cell's column with the adjacent
column; selection follows the moved column. Its header, width, color, sorting,
and filter stay attached. Moves stop at the table edges and apply outside filter
editing and help. Each transpose orientation remembers its own column order.

Ctrl-Shift-Left and Ctrl-Shift-Right shrink or grow the selected column; header `│`
handles can also be dragged to resize columns. Truncated cells end in a gray `…`.
Each transpose orientation remembers its own column widths.

## Sorting

Click a column header or press Ctrl-S to sort the selected cell's column. The
first sort is descending. Sorting the primary column again toggles its direction;
sorting a secondary column promotes it to primary without changing its direction,
and the following sort toggles it. Newly sorted columns become primary and retain
the existing columns as secondary sorts.

`▼` and `▲` mark primary descending and ascending sorts; `▽` and `△` mark their
secondary counterparts. Numeric columns containing only integers or decimals with
optional leading minus signs are ordered numerically.

## Filtering

Press Ctrl-F to show a filter row above the header. Each column has a one-line
`fancy-regex` field; Tab and Shift-Tab move between fields, Ctrl-A moves to the
beginning of the current filter line, and a mouse click focuses a field. Nonempty
fields are combined with AND and update automatically after a short debounce on
a background worker. Invalid expressions are shown in red and match no rows until
corrected.

Enter applies an edit immediately and releases textarea focus so ordinary viewer
keys work again. Clicking a data cell also releases focus while keeping filtering
active. Ctrl-F hides the fields and removes the filtering while remembering their
values; showing the row again reapplies them.

## Transpose

Press Ctrl-T to toggle between the original table and its transpose. Each toggle
uses all records in file order, clears sorting and filter values, closes the
filter row, and resets selection and scrolling to the top-left.

With headers, the first column becomes the new header and the original header
becomes the first column; `--no-header` keeps both orientations headerless. Missing
cells in ragged rows appear as empty strings in the transpose. Toggling back
restores the original rows exactly, and each orientation remembers its column
widths and order.

## Reset, help, and exit

Press Ctrl-R to clear all sorting and filter values, restore file order, and
cancel an active filter in the current orientation. Only Ctrl-T changes
orientation; resetting preserves column widths and order.

Press `?` outside an active filter textarea for the scrollable keymap. Escape acts
as Back: it closes help, then the filter row, then resets sorting, and finally
exits. Press `q` or `Q` to exit when neither a filter textarea nor the help dialog
has focus; Ctrl-C exits at any time.

Run `tuisv --help-keymap` to print the same keymap to the terminal and exit. No file
is required, and the output can be piped or redirected.

## Samples

The [`samples/`](../samples/) directory includes small files for trying the viewer:

- [`people.csv`](../samples/people.csv): headers, sorting, filtering, and transpose.
- [`numbers.csv`](../samples/numbers.csv): signed integers, decimals, and large
  values for numeric sorting.
- [`ragged.csv`](../samples/ragged.csv): short records and a cell beyond the header.
- [`headerless.csv`](../samples/headerless.csv): data without a header; use
  `--no-header`.
- [`text.csv`](../samples/text.csv): Unicode, quoted commas and quotes, embedded
  newlines, and a tab.
- [`people.tsv`](../samples/people.tsv): a tab-separated version of the people table.
- [`text.tsv`](../samples/text.tsv): commas in a header name and values, Unicode,
  quotes, embedded newlines, and a quoted tab.

```sh
cargo run -- samples/people.csv
cargo run -- samples/people.tsv
cargo run -- --tsv samples/text.tsv
cargo run -- --no-header samples/headerless.csv
```

## Builds and downloads

Prebuilt binaries are available on the [GitHub releases page](https://github.com/mevanlc/tuisv/releases)
for Linux (x64/ARM64, static musl), macOS (x64/ARM64), Windows (x64/ARM64), and
Android ARM64. Extract the archive and put `tuisv` (or `tuisv.exe`) on your PATH.
Android builds target API 24 or later and are intended for terminal environments
such as Termux; macOS builds target macOS 11 or later.

To build a checkout locally, run `cargo install --path . --locked`. Checkout
builds report `0.0.0`; release builds get their version from the Git tag.

Each release includes `SHA256SUMS`, the exact published crate, and a manifest
recording the source commit, toolchain, and asset hashes. See the
[release guide](../RELEASING.md) for verification, release operations, and adapting
this workflow to another project.
