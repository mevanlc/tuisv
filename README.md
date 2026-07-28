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

Press Ctrl-R to clear all sorting and filter values, restore file order, and
cancel an active filter. Press `?` outside an active filter textarea for the
scrollable keymap. Escape acts as Back: it closes help, then the filter row, then
resets sorting, and finally exits. Press `q` or `Q` to exit when neither a filter
textarea nor the help dialog has focus; Ctrl-C exits at any time.

The file is read fully into memory as UTF-8 CSV. Embedded control characters
are shown as escapes so each record occupies one terminal row.
