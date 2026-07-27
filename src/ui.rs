use ratatui::{
    Frame,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
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
}

fn render_filter_bar(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    if area.is_empty() {
        return;
    }

    let base_style = filter_style(false, false, false);
    frame.buffer_mut().set_style(area, base_style);
    let active_column = app.filter.active_column();
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
        let active = column == active_column;
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
        app.handle_event(key(KeyCode::Right));
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
