use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    app::{App, CellPosition},
    data::{COLUMN_GAP, CsvData},
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

pub fn render(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();
    app.set_viewport(area.width, area.height);
    if area.is_empty() {
        return;
    }

    if app.data.header.is_none() && app.data.rows.is_empty() {
        Paragraph::new("Empty CSV").render(area, frame.buffer_mut());
        return;
    }

    let mut screen_row = 0usize;
    if app.sticky_header
        && let Some(header) = &app.data.header
    {
        render_record(
            frame,
            row_area(area, screen_row),
            &app.data,
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
                &app.data,
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
            &app.data,
            record,
            app.selected.filter(|selected| selected.row == data_row),
            app.column_offset,
            Style::default(),
        );
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
    data: &CsvData,
    record: &[String],
    selected: Option<CellPosition>,
    horizontal_offset: usize,
    base_style: Style,
) {
    let line = record_line(data, record, selected, base_style);
    Paragraph::new(line)
        .style(base_style)
        .scroll((0, u16::try_from(horizontal_offset).unwrap_or(u16::MAX)))
        .render(area, frame.buffer_mut());
}

fn record_line<'a>(
    data: &CsvData,
    record: &'a [String],
    selected: Option<CellPosition>,
    base_style: Style,
) -> Line<'a> {
    let mut spans = Vec::with_capacity(data.column_count().saturating_mul(2));
    let is_header = base_style.bg == Some(Color::DarkGray);

    for column in 0..data.column_count() {
        let value = record.get(column).map_or("", String::as_str);
        let padding = data.widths[column].saturating_sub(UnicodeWidthStr::width(value));
        let style = if is_header {
            base_style
        } else {
            column_style(column, selected.is_some_and(|cell| cell.column == column))
        };
        spans.push(Span::styled(
            format!("{value}{}", " ".repeat(padding)),
            style,
        ));

        if column + 1 < data.column_count() {
            spans.push(Span::styled(" ".repeat(COLUMN_GAP), base_style));
        }
    }

    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend, style::Modifier};

    use super::*;

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
        assert_eq!(buffer[(0, 1)].fg, Color::Cyan);
        assert!(buffer[(0, 1)].modifier.contains(Modifier::REVERSED));
        assert_eq!(buffer[(7, 1)].fg, Color::Green);
        assert_eq!(buffer[(0, 2)].symbol(), "G");
        assert_eq!(buffer[(7, 2)].symbol(), "R");
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
