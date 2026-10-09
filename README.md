# tuisv

`tuisv` is a small, read-only CSV and TSV viewer for the terminal. It detects the
format and sizes columns to their contents. Sort, filter, transpose, and rearrange
the view without changing the file.

## Install

```sh
cargo install tuisv --locked
```

Or download a [prebuilt binary](https://github.com/mevanlc/tuisv/releases) for Linux,
macOS, or Windows (x64/ARM64), or Android ARM64, and put it on your PATH.

From a checkout: `cargo install --path . --locked`.

## Usage

```text
Usage: tuisv [OPTIONS] [FILE]

Arguments:
  [FILE]  CSV or TSV file to view

Options:
      --help-keymap       Print the interactive keymap and exit
      --csv               Force comma-separated input
      --tsv               Force tab-separated input
      --detect            Auto-detect CSV or TSV (the default)
      --header            Treat the first record as a header (the default)
      --no-header         Treat the first record as data instead of a header
      --sticky-header     Keep the header visible while scrolling (the default)
      --no-sticky-header  Scroll the header with the data instead of keeping it visible
      --sticky-leader     Keep column 1 visible while scrolling horizontally
      --no-sticky-leader  Scroll column 1 with the other columns (the default)
  -h, --help              Print help
  -V, --version           Print version
```

For paired header/sticky flags, the last flag wins. A file is required except when
printing help, the keymap, or the version. Try the files in [`samples/`](samples/):

```sh
tuisv samples/people.csv
tuisv --sticky-leader samples/people.tsv
tuisv --no-header samples/headerless.csv
```

## Keymap

Press `?` outside filter editing or run `tuisv --help-keymap` to see these bindings.
Column movement applies outside filter editing and help.

```text
General
?                 Open or close this keymap
Esc               Close help/filter, reset sorting, then exit
q / Q             Exit outside the help dialog and filter editing
Ctrl-C            Exit immediately
Ctrl-T            Transpose the full file; clear sorting and filters

Navigation
Arrow keys        Move the selected cell
Left-click cell   Select that cell and release filter focus
Mouse wheel       Pan without moving the selection
Shift-wheel       Pan horizontally

Sorting
Header click      Sort, promote, or toggle that column
Ctrl-S            Sort the selected cell's column
Ctrl-R            Reset sorting and filters

Filtering
Ctrl-F            Show or hide the filter row
Tab / Shift-Tab   Move between filter fields
Ctrl-A            Move to the beginning of the filter line
Enter             Apply the edit and release filter focus
Filter click      Focus that column's filter field

Columns
Shift-← / →       Move the selected column left or right
Ctrl-Shift-← / →  Shrink or grow the selected column
Drag header │     Resize that column
```

See the [manual](https://github.com/mevanlc/tuisv/blob/main/devdocs/MANUAL.md) for
format detection and detailed behavior, and the
[release guide](https://github.com/mevanlc/tuisv/blob/main/RELEASING.md) for download
verification and release operations.

## License

[MIT](LICENSE).
