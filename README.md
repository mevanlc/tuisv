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

Use the arrow keys to move the selected cell. The vertical and horizontal
mouse wheels pan the viewport without changing the selection; Shift-wheel is a
horizontal-scroll fallback. Click a column header or press Ctrl-S to sort the
selected cell's column. The first sort is descending; repeating a sorted column
toggles its direction, while newly sorted columns become primary and retain the
existing columns as secondary sorts. Numeric columns containing only integers or
decimals with optional leading minus signs are ordered numerically. Press Ctrl-R
to clear all sorting and restore file order. Press `q`, `Q`, or Ctrl-C to exit.

The file is read fully into memory as UTF-8 CSV. Embedded control characters
are shown as escapes so each record occupies one terminal row.
