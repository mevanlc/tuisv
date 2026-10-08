# tuisv

`tuisv` is a small, read-only CSV viewer for the terminal. It opens the file in
the alternate screen, sizes columns to their contents, and keeps the selected
cell visible during keyboard navigation.

```text
tuisv [OPTIONS] <CSVFILE>

Options:
      --no-header         Treat the first record as data
      --no-sticky-header  Scroll the header with the data
  -h, --help              Print help
  -V, --version           Print version
```

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

The file is read fully into memory as UTF-8 CSV. Embedded control characters
are shown as escapes so each record occupies one terminal row.

## Samples

The [`samples/`](samples/) directory includes small CSVs for trying the viewer:

- [`people.csv`](samples/people.csv): headers, sorting, filtering, and transpose.
- [`numbers.csv`](samples/numbers.csv): signed integers, decimals, and large values
  for numeric sorting.
- [`ragged.csv`](samples/ragged.csv): short records and a cell beyond the header.
- [`headerless.csv`](samples/headerless.csv): data without a header; use `--no-header`.
- [`text.csv`](samples/text.csv): Unicode, quoted commas and quotes, embedded
  newlines, and a tab.

```sh
cargo run -- samples/people.csv
cargo run -- --no-header samples/headerless.csv
```
