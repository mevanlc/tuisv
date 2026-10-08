use ratatui::{
    Frame,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph, Widget},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{
    app::{App, CellPosition},
    data::COLUMN_GAP,
};

const COLUMN_COLORS: [Color; 6] = [
    Color::Cyan,
    Color::Green,
    Color::Yellow,
    Color::Magenta,
    Color::Blue,
    Color::Red,
];

const HELP_MAX_WIDTH: u16 = 76;

#[derive(Clone, Copy)]
enum HelpEntry {
    Section(&'static str),
    Binding(&'static str, &'static str),
}

const HELP_ENTRIES: &[HelpEntry] = &[
    HelpEntry::Section("General"),
    HelpEntry::Binding("?", "Open or close this keymap"),
    HelpEntry::Binding("Esc", "Close help/filter, reset sorting, then exit"),
    HelpEntry::Binding("q / Q", "Exit outside the help dialog and filter editing"),
    HelpEntry::Binding("Ctrl-C", "Exit immediately"),
    HelpEntry::Binding(
        "Ctrl-T",
        "Transpose the full file; clear sorting and filters",
    ),
    HelpEntry::Section("Navigation"),
    HelpEntry::Binding("Arrow keys", "Move the selected cell"),
    HelpEntry::Binding("Mouse wheel", "Pan without moving the selection"),
    HelpEntry::Binding("Shift-wheel", "Pan horizontally"),
    HelpEntry::Section("Sorting"),
    HelpEntry::Binding("Header click", "Sort, promote, or toggle that column"),
    HelpEntry::Binding("Ctrl-S", "Sort the selected cell's column"),
    HelpEntry::Binding("Ctrl-R", "Reset sorting and filters"),
    HelpEntry::Section("Filtering"),
    HelpEntry::Binding("Ctrl-F", "Show or hide the filter row"),
    HelpEntry::Binding("Tab / Shift-Tab", "Move between filter fields"),
    HelpEntry::Binding("Ctrl-A", "Move to the beginning of the filter line"),
    HelpEntry::Binding("Enter", "Apply the edit and release filter focus"),
    HelpEntry::Binding("Filter click", "Focus that column's filter field"),
    HelpEntry::Section("Column width"),
    HelpEntry::Binding("Ctrl-Shift-← / →", "Shrink or grow the selected column"),
    HelpEntry::Binding("Drag header │", "Resize that column"),
];

fn header_style() -> Style {
    Style::new()
        .fg(Color::White)
        .bg(Color::DarkGray)
        .add_modifier(Modifier::BOLD)
}

fn column_style(column: usize, selected: bool) -> Style {
    let style = Style::new().fg(COLUMN_COLORS[column % COLUMN_COLORS.len()]);
    if selected {
        style.add_modifier(Modifier::BOLD | Modifier::REVERSED)
    } else {
        style
    }
}

fn filter_style(active: bool, error: bool, processing: bool) -> Style {
    let foreground = if error {
        Color::LightRed
    } else if processing {
        Color::Yellow
    } else {
        Color::White
    };
    let background = if active {
        Color::Rgb(55, 55, 55)
    } else {
        Color::Rgb(28, 28, 28)
    };
    Style::new().fg(foreground).bg(background)
}

pub fn render(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();
    app.set_viewport(area.width, area.height);
    if area.is_empty() {
        return;
    }

    let mut screen_row = 0usize;
    if app.filter.is_visible() {
        render_filter_bar(frame, row_area(area, screen_row), app);
        screen_row += 1;
    }

    if app.data.header.is_none() && app.data.rows.is_empty() {
        if screen_row < usize::from(area.height) {
            Paragraph::new("Empty CSV").render(row_area(area, screen_row), frame.buffer_mut());
        }
        if app.help_visible() {
            render_help(frame, app);
        }
        return;
    }

    if app.sticky_header
        && let Some(header) = &app.data.header
        && screen_row < usize::from(area.height)
    {
        render_record(
            frame,
            row_area(area, screen_row),
            app,
            header,
            None,
            app.column_offset,
            header_style(),
        );
        screen_row += 1;
    }

    let available_rows = usize::from(area.height).saturating_sub(screen_row);
    for visible_index in 0..available_rows {
        let visual_row = app.row_offset + visible_index;
        let area = row_area(area, screen_row + visible_index);

        if !app.sticky_header && app.data.header.is_some() && visual_row == 0 {
            render_record(
                frame,
                area,
                app,
                app.data.header.as_deref().unwrap_or_default(),
                None,
                app.column_offset,
                header_style(),
            );
            continue;
        }

        let data_row =
            visual_row.saturating_sub(usize::from(!app.sticky_header && app.data.header.is_some()));
        let Some(record) = app.displayed_row(data_row) else {
            break;
        };
        render_record(
            frame,
            area,
            app,
            record,
            app.selected.filter(|selected| selected.row == data_row),
            app.column_offset,
            Style::default(),
        );
    }

    if app.help_visible() {
        render_help(frame, app);
    }
}

fn render_filter_bar(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    if area.is_empty() {
        return;
    }

    let base_style = filter_style(false, false, false);
    frame.buffer_mut().set_style(area, base_style);
    let active_column = app.filter.active_column();
    let editing = app.filter.is_editing();
    let processing = app.filter.is_processing();
    let horizontal_offset = app.column_offset;
    let viewport_start = horizontal_offset;
    let viewport_end = viewport_start.saturating_add(usize::from(area.width));

    for column in 0..app.data.column_count() {
        let column_start = app.data.column_starts[column];
        let column_width = app.data.widths[column];
        let column_end = column_start.saturating_add(column_width);
        let visible_start = column_start.max(viewport_start);
        let visible_end = column_end.min(viewport_end);
        let active = editing && column == active_column;
        let style = filter_style(active, app.filter.has_error(column), processing && active);

        if visible_start < visible_end {
            let local_width = u16::try_from(column_width).unwrap_or(u16::MAX);
            let local_area = Rect::new(0, 0, local_width, 1);
            let mut local = Buffer::empty(local_area);
            if let Some(editor) = app.filter.editor_mut(column) {
                editor.set_style(style);
                editor.set_cursor_line_style(Style::default());
                editor.set_cursor_style(if active {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                });
                (&*editor).render(local_area, &mut local);
            }

            for content_column in visible_start..visible_end {
                let source_x = u16::try_from(content_column - column_start).unwrap_or(u16::MAX);
                let target_x = area.x.saturating_add(
                    u16::try_from(content_column - viewport_start).unwrap_or(u16::MAX),
                );
                frame.buffer_mut()[(target_x, area.y)] = local[(source_x, 0)].clone();
            }
        }

        let separator_column = column_end.saturating_add(1);
        if (viewport_start..viewport_end).contains(&separator_column) {
            let target_x = area.x.saturating_add(
                u16::try_from(separator_column - viewport_start).unwrap_or(u16::MAX),
            );
            frame.buffer_mut()[(target_x, area.y)]
                .set_symbol("│")
                .set_style(style.fg(Color::Gray));
        }
    }
}

pub(crate) fn help_max_scroll(viewport_width: usize, viewport_height: usize) -> usize {
    let area = Rect::new(
        0,
        0,
        u16::try_from(viewport_width).unwrap_or(u16::MAX),
        u16::try_from(viewport_height).unwrap_or(u16::MAX),
    );
    let popup = help_popup_area(area);
    let inner_width = popup.width.saturating_sub(2);
    let inner_height = usize::from(popup.height.saturating_sub(2));
    help_lines(inner_width).len().saturating_sub(inner_height)
}

fn render_help(frame: &mut Frame<'_>, app: &mut App) {
    let popup = help_popup_area(frame.area());
    if popup.is_empty() {
        return;
    }

    Clear.render(popup, frame.buffer_mut());
    let block = Block::bordered()
        .border_style(Style::new().fg(Color::LightMagenta))
        .title(Span::styled(
            " Keymap ",
            Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ))
        .title_bottom(Span::styled(
            " Arrows scroll · ?/Esc close ",
            Style::new().fg(Color::DarkGray),
        ));
    let inner = block.inner(popup);
    block.render(popup, frame.buffer_mut());

    let lines = help_lines(inner.width);
    let maximum_scroll = lines.len().saturating_sub(usize::from(inner.height));
    app.clamp_help_scroll(maximum_scroll);
    Paragraph::new(lines)
        .scroll((u16::try_from(app.help_scroll()).unwrap_or(u16::MAX), 0))
        .render(inner, frame.buffer_mut());
}

fn help_popup_area(area: Rect) -> Rect {
    if area.is_empty() {
        return area;
    }
    let available_width = if area.width > 4 {
        area.width - 4
    } else {
        area.width
    };
    let width = available_width.min(HELP_MAX_WIDTH);
    let inner_width = width.saturating_sub(2);
    let desired_height =
        u16::try_from(help_lines(inner_width).len().saturating_add(2)).unwrap_or(u16::MAX);
    let available_height = if area.height > 2 {
        area.height - 2
    } else {
        area.height
    };
    let height = desired_height.min(available_height);
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    )
}

fn help_lines(width: u16) -> Vec<Line<'static>> {
    let width = usize::from(width);
    if width == 0 {
        return Vec::new();
    }

    let compact = width < 36;
    let key_width = 19usize.min(width.saturating_sub(1));
    let description_width = width.saturating_sub(key_width);
    let mut lines = Vec::new();
    for entry in HELP_ENTRIES {
        match *entry {
            HelpEntry::Section(section) => lines.push(Line::from(Span::styled(
                section,
                Style::new()
                    .fg(Color::LightMagenta)
                    .add_modifier(Modifier::BOLD),
            ))),
            HelpEntry::Binding(key, description) if compact => {
                lines.push(Line::from(Span::styled(
                    key,
                    Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                )));
                for part in wrap_words(description, width.saturating_sub(2).max(1)) {
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(part, Style::new().fg(Color::White)),
                    ]));
                }
            }
            HelpEntry::Binding(key, description) => {
                let parts = wrap_words(description, description_width.max(1));
                for (index, part) in parts.into_iter().enumerate() {
                    let key = if index == 0 { key } else { "" };
                    lines.push(Line::from(vec![
                        Span::styled(
                            pad_to_width(key, key_width),
                            Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(part, Style::new().fg(Color::White)),
                    ]));
                }
            }
        }
    }
    lines
}

fn wrap_words(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let separator = usize::from(!line.is_empty());
        if !line.is_empty()
            && UnicodeWidthStr::width(line.as_str())
                .saturating_add(separator)
                .saturating_add(UnicodeWidthStr::width(word))
                > width
        {
            lines.push(line);
            line = String::new();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn pad_to_width(value: &str, width: usize) -> String {
    let value_width = UnicodeWidthStr::width(value);
    format!("{value}{}", " ".repeat(width.saturating_sub(value_width)))
}

fn row_area(area: Rect, row: usize) -> Rect {
    Rect::new(
        area.x,
        area.y
            .saturating_add(u16::try_from(row).unwrap_or(u16::MAX)),
        area.width,
        1,
    )
}

fn render_record(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    record: &[String],
    selected: Option<CellPosition>,
    horizontal_offset: usize,
    base_style: Style,
) {
    let line = record_line(app, record, selected, base_style);
    Paragraph::new(line)
        .style(base_style)
        .scroll((0, u16::try_from(horizontal_offset).unwrap_or(u16::MAX)))
        .render(area, frame.buffer_mut());
}

fn record_line(
    app: &App,
    record: &[String],
    selected: Option<CellPosition>,
    base_style: Style,
) -> Line<'static> {
    let data = &app.data;
    let mut spans = Vec::with_capacity(data.column_count().saturating_mul(3));
    let is_header = base_style.bg == Some(Color::DarkGray);

    for column in 0..data.column_count() {
        let value = record.get(column).map_or("", String::as_str);
        let style = if is_header {
            base_style
        } else {
            column_style(column, selected.is_some_and(|cell| cell.column == column))
        };
        push_cell(&mut spans, value, data.widths[column], style);

        if is_header {
            spans.push(Span::styled(
                app.sort_indicator(column).unwrap_or(' ').to_string(),
                base_style,
            ));
            spans.push(Span::styled("│", base_style.fg(Color::Gray)));
        } else {
            spans.push(Span::styled(" ".repeat(COLUMN_GAP), base_style));
        }
    }

    Line::from(spans)
}

fn push_cell(spans: &mut Vec<Span<'static>>, value: &str, width: usize, style: Style) {
    let value_width = UnicodeWidthStr::width(value);
    if value_width <= width {
        spans.push(Span::styled(
            format!("{value}{}", " ".repeat(width - value_width)),
            style,
        ));
        return;
    }

    let prefix_width = width.saturating_sub(1);
    let mut prefix = String::new();
    let mut displayed_width = 0usize;
    for character in value.chars() {
        let character_width = UnicodeWidthChar::width(character).unwrap_or(0);
        if displayed_width.saturating_add(character_width) > prefix_width {
            break;
        }
        prefix.push(character);
        displayed_width = displayed_width.saturating_add(character_width);
    }
    prefix.push_str(&" ".repeat(prefix_width.saturating_sub(displayed_width)));
    if !prefix.is_empty() {
        spans.push(Span::styled(prefix, style));
    }
    let ellipsis_style = if style.add_modifier.contains(Modifier::REVERSED) {
        style.bg(Color::Gray)
    } else {
        style.fg(Color::Gray)
    };
    spans.push(Span::styled("…", ellipsis_style));
}

#[cfg(test)]
mod tests {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{Terminal, backend::TestBackend, style::Modifier};

    use super::*;
    use crate::data::CsvData;

    fn control_key(character: char) -> Event {
        Event::Key(KeyEvent::new(
            KeyCode::Char(character),
            KeyModifiers::CONTROL,
        ))
    }

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn render(csv: &str, has_header: bool, sticky_header: bool, width: u16, height: u16) -> App {
        let data = CsvData::from_reader(csv.as_bytes(), has_header).unwrap();
        let mut app = App::new(data, sticky_header);
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| super::render(frame, &mut app))
            .unwrap();
        app
    }

    #[test]
    fn renders_aligned_colored_columns_and_selected_cell() {
        let data =
            CsvData::from_reader("name,city\nAda,London\nGrace,Rome\n".as_bytes(), true).unwrap();
        let mut app = App::new(data, true);
        let backend = TestBackend::new(20, 4);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| super::render(frame, &mut app))
            .unwrap();
        let buffer = terminal.backend().buffer();

        assert_eq!(buffer[(0, 0)].symbol(), "n");
        assert_eq!(buffer[(0, 0)].bg, Color::DarkGray);
        assert!(buffer[(0, 0)].modifier.contains(Modifier::BOLD));
        assert_eq!(buffer[(6, 0)].symbol(), "│");
        assert_eq!(buffer[(0, 1)].fg, Color::Cyan);
        assert!(buffer[(0, 1)].modifier.contains(Modifier::REVERSED));
        assert_eq!(buffer[(7, 1)].fg, Color::Green);
        assert_eq!(buffer[(0, 2)].symbol(), "G");
        assert_eq!(buffer[(7, 2)].symbol(), "R");
    }

    #[test]
    fn transposed_tables_render_and_navigate_with_each_header_mode() {
        for (has_header, sticky_header) in [(true, true), (true, false), (false, false)] {
            let data = CsvData::from_reader(
                "name,age,city\nAda,37,London\nBob,42,Rome\n".as_bytes(),
                has_header,
            )
            .unwrap();
            let mut app = App::new(data, sticky_header);
            app.handle_event(control_key('t'));
            let backend = TestBackend::new(24, 2);
            let mut terminal = Terminal::new(backend).unwrap();

            terminal
                .draw(|frame| super::render(frame, &mut app))
                .unwrap();
            let buffer = terminal.backend().buffer();
            assert_eq!(buffer[(0, 0)].symbol(), "n");
            assert_eq!(buffer[(6, 0)].symbol(), "A");
            assert_eq!(buffer[(14, 0)].symbol(), "B");
            assert_eq!(buffer[(0, 0)].bg == Color::DarkGray, has_header);
            let selected_row = u16::from(has_header);
            assert!(
                buffer[(0, selected_row)]
                    .modifier
                    .contains(Modifier::REVERSED)
            );
            assert_eq!(buffer[(6, 1)].symbol(), "3");

            app.handle_event(key(KeyCode::Down));
            terminal
                .draw(|frame| super::render(frame, &mut app))
                .unwrap();
            let buffer = terminal.backend().buffer();
            assert_eq!(
                buffer[(0, 0)].symbol(),
                if has_header && !sticky_header {
                    "a"
                } else {
                    "n"
                }
            );
            assert_eq!(buffer[(0, 1)].symbol(), if has_header { "c" } else { "a" });
            assert!(buffer[(0, 1)].modifier.contains(Modifier::REVERSED));

            app.handle_event(control_key('t'));
            terminal
                .draw(|frame| super::render(frame, &mut app))
                .unwrap();
            let buffer = terminal.backend().buffer();
            assert_eq!(buffer[(0, 0)].symbol(), "n");
            assert_eq!(buffer[(0, 1)].symbol(), "A");
        }
    }

    #[test]
    fn narrow_columns_end_truncated_values_with_a_gray_ellipsis() {
        let data = CsvData::from_reader("header,other\nabcdefgh,z\nijklmnop,y\n".as_bytes(), true)
            .unwrap();
        let mut app = App::new(data, true);
        app.data.set_column_width(0, 4);
        let backend = TestBackend::new(20, 4);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| super::render(frame, &mut app))
            .unwrap();
        let buffer = terminal.backend().buffer();

        assert_eq!(buffer[(0, 0)].symbol(), "h");
        assert_eq!(buffer[(3, 0)].symbol(), "…");
        assert_eq!(buffer[(3, 0)].fg, Color::Gray);
        assert_eq!(buffer[(5, 0)].symbol(), "│");
        assert_eq!(buffer[(0, 1)].symbol(), "a");
        assert_eq!(buffer[(3, 1)].symbol(), "…");
        assert_eq!(buffer[(3, 1)].bg, Color::Gray);
        assert!(buffer[(3, 1)].modifier.contains(Modifier::REVERSED));
        assert_eq!(buffer[(6, 1)].symbol(), "z");
        assert_eq!(buffer[(3, 2)].symbol(), "…");
        assert_eq!(buffer[(3, 2)].fg, Color::Gray);
    }

    #[test]
    fn truncation_does_not_split_a_wide_character() {
        let data = CsvData::from_reader("header\n界x\n".as_bytes(), true).unwrap();
        let mut app = App::new(data, true);
        app.data.set_column_width(0, 2);
        let backend = TestBackend::new(6, 2);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| super::render(frame, &mut app))
            .unwrap();
        let buffer = terminal.backend().buffer();

        assert_eq!(buffer[(0, 1)].symbol(), " ");
        assert_eq!(buffer[(1, 1)].symbol(), "…");
    }

    #[test]
    fn headers_render_primary_and_secondary_sort_directions() {
        let data = CsvData::from_reader("first,second\nb,2\na,1\n".as_bytes(), true).unwrap();
        let mut app = App::new(data, true);
        app.handle_event(control_key('s'));
        app.handle_event(key(KeyCode::Right));
        app.handle_event(control_key('s'));
        let backend = TestBackend::new(18, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| super::render(frame, &mut app))
            .unwrap();
        assert_eq!(terminal.backend().buffer()[(5, 0)].symbol(), "▽");
        assert_eq!(terminal.backend().buffer()[(6, 0)].symbol(), "│");
        assert_eq!(terminal.backend().buffer()[(13, 0)].symbol(), "▼");
        assert_eq!(terminal.backend().buffer()[(14, 0)].symbol(), "│");

        app.handle_event(key(KeyCode::Left));
        app.handle_event(control_key('s'));
        app.handle_event(control_key('s'));
        app.handle_event(key(KeyCode::Right));
        app.handle_event(control_key('s'));
        app.handle_event(control_key('s'));
        terminal
            .draw(|frame| super::render(frame, &mut app))
            .unwrap();
        assert_eq!(terminal.backend().buffer()[(5, 0)].symbol(), "△");
        assert_eq!(terminal.backend().buffer()[(13, 0)].symbol(), "▲");
    }

    #[test]
    fn filter_bar_renders_textareas_above_the_shifted_header() {
        let data =
            CsvData::from_reader("name,city\nAda,London\nGrace,Rome\n".as_bytes(), true).unwrap();
        let mut app = App::new(data, true);
        app.handle_event(control_key('f'));
        app.handle_event(key(KeyCode::Char('^')));
        app.handle_event(key(KeyCode::Char('A')));
        let backend = TestBackend::new(20, 4);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| super::render(frame, &mut app))
            .unwrap();
        let buffer = terminal.backend().buffer();

        assert_eq!(buffer[(0, 0)].symbol(), "^");
        assert_eq!(buffer[(1, 0)].symbol(), "A");
        assert_eq!(buffer[(0, 0)].bg, Color::Rgb(55, 55, 55));
        assert_eq!(buffer[(6, 0)].symbol(), "│");
        assert_eq!(buffer[(0, 1)].symbol(), "n");
        assert_eq!(buffer[(0, 1)].bg, Color::DarkGray);
        assert_eq!(buffer[(0, 2)].symbol(), "A");
    }

    #[test]
    fn help_dialog_is_centered_column_aligned_and_color_accented() {
        let data = CsvData::from_reader("value\na\n".as_bytes(), true).unwrap();
        let mut app = App::new(data, true);
        app.handle_event(key(KeyCode::Char('?')));
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| super::render(frame, &mut app))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let screen = buffer
            .content
            .chunks(80)
            .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        let popup = help_popup_area(Rect::new(0, 0, 80, 24));

        assert_eq!(buffer[(popup.x, popup.y)].symbol(), "┌");
        assert_eq!(buffer[(popup.x, popup.y)].fg, Color::LightMagenta);
        assert!(screen.contains("Keymap"));
        assert!(screen.contains("General"));
        assert!(
            screen
                .contains("Ctrl-T             Transpose the full file; clear sorting and filters")
        );
        assert!(screen.contains("Ctrl-F             Show or hide the filter row"));
        assert!(
            buffer
                .content
                .iter()
                .any(|cell| cell.symbol() == "G" && cell.fg == Color::LightMagenta)
        );
        assert!(
            buffer
                .content
                .iter()
                .any(|cell| cell.symbol() == "C" && cell.fg == Color::Cyan)
        );
    }

    #[test]
    fn short_help_dialog_scrolls_its_contents() {
        let data = CsvData::from_reader("value\na\n".as_bytes(), true).unwrap();
        let mut app = App::new(data, true);
        app.set_viewport(42, 8);
        app.handle_event(key(KeyCode::Char('?')));
        let backend = TestBackend::new(42, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| super::render(frame, &mut app))
            .unwrap();
        let before = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(before.contains("General"));

        for _ in 0..3 {
            app.handle_event(key(KeyCode::Down));
        }
        terminal
            .draw(|frame| super::render(frame, &mut app))
            .unwrap();
        let after = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert_eq!(app.help_scroll(), 3);
        assert!(!after.contains("General"));
        assert_ne!(before, after);
    }

    #[test]
    fn horizontal_offset_clips_inside_a_column() {
        let data = CsvData::from_reader("header\nabcdefghij\n".as_bytes(), true).unwrap();
        let mut app = App::new(data, true);
        app.column_offset = 4;
        let backend = TestBackend::new(5, 2);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| super::render(frame, &mut app))
            .unwrap();
        let body = terminal.backend().buffer().content[5..10]
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert_eq!(body, "efghi");
    }

    #[test]
    fn sticky_and_scrolling_headers_use_different_rows() {
        let sticky = render("header\na\nb\nc\n", true, true, 10, 2);
        assert_eq!(sticky.scrollable_height(), 1);

        let scrolling = render("header\na\nb\nc\n", true, false, 10, 2);
        assert_eq!(scrolling.scrollable_height(), 2);
        assert_eq!(scrolling.visual_row_for_data(0), 1);
    }

    #[test]
    fn empty_file_renders_message_without_selection() {
        let data = CsvData::from_reader("".as_bytes(), true).unwrap();
        let mut app = App::new(data, true);
        let backend = TestBackend::new(12, 2);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| super::render(frame, &mut app))
            .unwrap();

        assert_eq!(terminal.backend().buffer()[(0, 0)].symbol(), "E");
        assert_eq!(app.selected, None);
    }
}
