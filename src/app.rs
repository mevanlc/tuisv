use std::{io, io::stdout, panic};

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    execute,
};
use ratatui::DefaultTerminal;

use crate::{data::CsvData, sort::SortState, ui};

const VERTICAL_SCROLL_AMOUNT: usize = 3;
const HORIZONTAL_SCROLL_AMOUNT: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellPosition {
    pub row: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ColumnResizeDrag {
    column: usize,
    initial_screen_column: u16,
    initial_width: usize,
}

#[derive(Debug)]
pub struct App {
    pub data: CsvData,
    pub sticky_header: bool,
    pub selected: Option<CellPosition>,
    pub row_offset: usize,
    pub column_offset: usize,
    sort: SortState,
    column_resize_drag: Option<ColumnResizeDrag>,
    viewport_width: usize,
    viewport_height: usize,
    quit: bool,
}

impl App {
    pub fn new(data: CsvData, sticky_header: bool) -> Self {
        let selected = (!data.rows.is_empty() && data.column_count() > 0)
            .then_some(CellPosition { row: 0, column: 0 });
        let sort = SortState::new(data.rows.len());
        Self {
            data,
            sticky_header,
            selected,
            row_offset: 0,
            column_offset: 0,
            sort,
            column_resize_drag: None,
            viewport_width: 0,
            viewport_height: 0,
            quit: false,
        }
    }

    pub fn set_viewport(&mut self, width: u16, height: u16) {
        self.viewport_width = usize::from(width);
        self.viewport_height = usize::from(height);
        self.clamp_offsets();
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    pub fn handle_event(&mut self, event: Event) {
        match event {
            Event::Key(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            Event::Resize(width, height) => self.set_viewport(width, height),
            _ => {}
        }
    }

    pub fn scrollable_height(&self) -> usize {
        if self.sticky_header && self.data.header.is_some() {
            self.viewport_height.saturating_sub(1)
        } else {
            self.viewport_height
        }
    }

    pub fn scrollable_row_count(&self) -> usize {
        self.data.rows.len() + usize::from(!self.sticky_header && self.data.header.is_some())
    }

    pub fn visual_row_for_data(&self, data_row: usize) -> usize {
        data_row + usize::from(!self.sticky_header && self.data.header.is_some())
    }

    pub(crate) fn displayed_row(&self, displayed_row: usize) -> Option<&[String]> {
        self.sort
            .displayed_row_index(displayed_row)
            .and_then(|row| self.data.rows.get(row))
            .map(Vec::as_slice)
    }

    pub(crate) fn sort_indicator(&self, column: usize) -> Option<char> {
        self.sort.indicator(column)
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return;
        }

        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => self.quit = true,
            KeyCode::Char('s' | 'S') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(selected) = self.selected {
                    self.sort.toggle_column(&self.data, selected.column);
                }
            }
            KeyCode::Char('r' | 'R') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.sort.reset();
            }
            KeyCode::Char('q' | 'Q') => self.quit = true,
            KeyCode::Left if key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT) => {
                self.resize_selected_column(-1);
            }
            KeyCode::Right if key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT) => {
                self.resize_selected_column(1);
            }
            KeyCode::Up => self.move_selection(-1, 0),
            KeyCode::Down => self.move_selection(1, 0),
            KeyCode::Left => self.move_selection(0, -1),
            KeyCode::Right => self.move_selection(0, 1),
            _ => {}
        }
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(column) = self.resize_handle_at(mouse.column, mouse.row) {
                    self.column_resize_drag = Some(ColumnResizeDrag {
                        column,
                        initial_screen_column: mouse.column,
                        initial_width: self.data.widths[column],
                    });
                } else {
                    self.column_resize_drag = None;
                    self.sort_from_header_click(mouse.column, mouse.row);
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                self.resize_dragged_column(mouse.column);
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.column_resize_drag = None;
            }
            MouseEventKind::ScrollUp if mouse.modifiers.contains(KeyModifiers::SHIFT) => {
                self.column_resize_drag = None;
                self.pan_horizontal(-(HORIZONTAL_SCROLL_AMOUNT as isize));
            }
            MouseEventKind::ScrollDown if mouse.modifiers.contains(KeyModifiers::SHIFT) => {
                self.column_resize_drag = None;
                self.pan_horizontal(HORIZONTAL_SCROLL_AMOUNT as isize);
            }
            MouseEventKind::ScrollUp => {
                self.column_resize_drag = None;
                self.pan_vertical(-(VERTICAL_SCROLL_AMOUNT as isize));
            }
            MouseEventKind::ScrollDown => {
                self.column_resize_drag = None;
                self.pan_vertical(VERTICAL_SCROLL_AMOUNT as isize);
            }
            MouseEventKind::ScrollLeft => {
                self.column_resize_drag = None;
                self.pan_horizontal(-(HORIZONTAL_SCROLL_AMOUNT as isize));
            }
            MouseEventKind::ScrollRight => {
                self.column_resize_drag = None;
                self.pan_horizontal(HORIZONTAL_SCROLL_AMOUNT as isize);
            }
            _ => {}
        }
    }

    fn sort_from_header_click(&mut self, screen_column: u16, screen_row: u16) {
        let Some(content_column) = self.header_content_column(screen_column, screen_row) else {
            return;
        };
        let clicked_column = self
            .data
            .column_starts
            .iter()
            .zip(&self.data.widths)
            .position(|(start, width)| {
                content_column >= *start && content_column <= start.saturating_add(*width)
            });
        if let Some(column) = clicked_column {
            self.sort.toggle_column(&self.data, column);
        }
    }

    fn resize_handle_at(&self, screen_column: u16, screen_row: u16) -> Option<usize> {
        let content_column = self.header_content_column(screen_column, screen_row)?;
        self.data
            .column_starts
            .iter()
            .zip(&self.data.widths)
            .position(|(start, width)| {
                content_column == start.saturating_add(*width).saturating_add(1)
            })
    }

    fn header_content_column(&self, screen_column: u16, screen_row: u16) -> Option<usize> {
        (self.data.header.is_some()
            && (self.sticky_header || self.row_offset == 0)
            && screen_row == 0)
            .then(|| {
                self.column_offset
                    .saturating_add(usize::from(screen_column))
            })
    }

    fn resize_selected_column(&mut self, delta: isize) {
        let Some(selected) = self.selected else {
            return;
        };
        self.data.resize_column(selected.column, delta);
        self.ensure_selection_visible();
    }

    fn resize_dragged_column(&mut self, screen_column: u16) {
        let Some(drag) = self.column_resize_drag else {
            return;
        };
        let width = if screen_column >= drag.initial_screen_column {
            drag.initial_width
                .saturating_add(usize::from(screen_column - drag.initial_screen_column))
        } else {
            drag.initial_width
                .saturating_sub(usize::from(drag.initial_screen_column - screen_column))
        };
        self.data.set_column_width(drag.column, width);
        self.clamp_offsets();
    }

    fn move_selection(&mut self, row_delta: isize, column_delta: isize) {
        let Some(mut selected) = self.selected else {
            return;
        };

        selected.row = selected
            .row
            .saturating_add_signed(row_delta)
            .min(self.data.rows.len().saturating_sub(1));
        selected.column = selected
            .column
            .saturating_add_signed(column_delta)
            .min(self.data.column_count().saturating_sub(1));
        self.selected = Some(selected);
        self.ensure_selection_visible();
    }

    fn pan_vertical(&mut self, amount: isize) {
        self.row_offset = self.row_offset.saturating_add_signed(amount);
        self.clamp_offsets();
    }

    fn pan_horizontal(&mut self, amount: isize) {
        self.column_offset = self.column_offset.saturating_add_signed(amount);
        self.clamp_offsets();
    }

    fn ensure_selection_visible(&mut self) {
        let Some(selected) = self.selected else {
            return;
        };

        let visible_height = self.scrollable_height();
        let visual_row = self.visual_row_for_data(selected.row);
        if visual_row < self.row_offset {
            self.row_offset = visual_row;
        } else if visible_height > 0 && visual_row >= self.row_offset + visible_height {
            self.row_offset = visual_row + 1 - visible_height;
        }

        let column_start = self.data.column_starts[selected.column];
        let column_width = self.data.widths[selected.column];
        let column_end = column_start.saturating_add(column_width);
        if column_start < self.column_offset || column_width > self.viewport_width {
            self.column_offset = column_start;
        } else if self.viewport_width > 0 && column_end > self.column_offset + self.viewport_width {
            self.column_offset = column_end - self.viewport_width;
        }

        self.clamp_offsets();
    }

    fn clamp_offsets(&mut self) {
        let maximum_row_offset = self
            .scrollable_row_count()
            .saturating_sub(self.scrollable_height());
        self.row_offset = self.row_offset.min(maximum_row_offset);

        let maximum_column_offset = self.data.content_width.saturating_sub(self.viewport_width);
        self.column_offset = self.column_offset.min(maximum_column_offset);
    }
}

pub struct TerminalSession {
    terminal: DefaultTerminal,
}

impl TerminalSession {
    pub fn start() -> io::Result<Self> {
        let terminal = ratatui::try_init()?;
        if let Err(error) = execute!(stdout(), EnableMouseCapture) {
            ratatui::restore();
            return Err(error);
        }

        let previous_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic_info| {
            let _ = execute!(stdout(), DisableMouseCapture);
            previous_hook(panic_info);
        }));

        Ok(Self { terminal })
    }

    pub fn run(&mut self, app: &mut App) -> io::Result<()> {
        while !app.should_quit() {
            self.terminal.draw(|frame| ui::render(frame, app))?;
            app.handle_event(event::read()?);
        }
        Ok(())
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = execute!(stdout(), DisableMouseCapture);
        ratatui::restore();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(csv: &str, has_header: bool, sticky_header: bool) -> App {
        App::new(
            CsvData::from_reader(csv.as_bytes(), has_header).unwrap(),
            sticky_header,
        )
    }

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn modified_key(code: KeyCode, modifiers: KeyModifiers) -> Event {
        Event::Key(KeyEvent::new(code, modifiers))
    }

    fn wheel(kind: MouseEventKind, modifiers: KeyModifiers) -> Event {
        Event::Mouse(MouseEvent {
            kind,
            column: 0,
            row: 0,
            modifiers,
        })
    }

    fn mouse(kind: MouseEventKind, column: u16, row: u16) -> Event {
        Event::Mouse(MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        })
    }

    fn click(column: u16, row: u16) -> Event {
        mouse(MouseEventKind::Down(MouseButton::Left), column, row)
    }

    fn displayed_column(app: &App, column: usize) -> Vec<String> {
        (0..app.data.rows.len())
            .map(|row| app.displayed_row(row).unwrap()[column].clone())
            .collect()
    }

    #[test]
    fn arrows_clamp_and_scroll_at_viewport_edges() {
        let mut app = app("h1,h2,h3\na,b,c\nd,e,f\ng,h,i\n", true, true);
        app.set_viewport(3, 3);

        app.handle_event(key(KeyCode::Down));
        app.handle_event(key(KeyCode::Down));
        app.handle_event(key(KeyCode::Down));
        assert_eq!(app.selected.unwrap().row, 2);
        assert_eq!(app.row_offset, 1);

        app.handle_event(key(KeyCode::Right));
        app.handle_event(key(KeyCode::Right));
        app.handle_event(key(KeyCode::Right));
        assert_eq!(app.selected.unwrap().column, 2);
        assert_eq!(app.column_offset, 7);

        app.handle_event(key(KeyCode::Left));
        app.handle_event(key(KeyCode::Left));
        assert_eq!(app.column_offset, 0);
    }

    #[test]
    fn mouse_pans_without_moving_selection() {
        let mut app = app("header-a,header-b\n1,2\n3,4\n5,6\n7,8\n9,10\n", true, true);
        app.set_viewport(5, 3);
        let selected = app.selected;

        app.handle_event(wheel(MouseEventKind::ScrollDown, KeyModifiers::NONE));
        app.handle_event(wheel(MouseEventKind::ScrollRight, KeyModifiers::NONE));

        assert_eq!(app.selected, selected);
        assert_eq!(app.row_offset, 3);
        assert_eq!(app.column_offset, 4);
    }

    #[test]
    fn shift_vertical_wheel_pans_horizontally() {
        let mut app = app("long-header,second-header\na,b\n", true, true);
        app.set_viewport(5, 3);

        app.handle_event(wheel(MouseEventKind::ScrollDown, KeyModifiers::SHIFT));
        assert_eq!(app.column_offset, 4);
        assert_eq!(app.row_offset, 0);
    }

    #[test]
    fn scrolling_header_participates_in_visual_rows() {
        let mut app = app("header\na\nb\nc\n", true, false);
        app.set_viewport(10, 2);
        app.handle_event(wheel(MouseEventKind::ScrollDown, KeyModifiers::NONE));

        assert_eq!(app.scrollable_row_count(), 4);
        assert_eq!(app.row_offset, 2);
        assert_eq!(app.visual_row_for_data(0), 1);
    }

    #[test]
    fn control_s_sorts_the_selected_column_and_control_r_resets() {
        let mut app = app("name,number\na,2\nb,10\nc,1\n", true, true);
        app.handle_event(key(KeyCode::Right));

        app.handle_event(modified_key(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert_eq!(displayed_column(&app, 0), ["b", "a", "c"]);

        app.handle_event(modified_key(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert_eq!(displayed_column(&app, 0), ["c", "a", "b"]);

        app.handle_event(modified_key(KeyCode::Char('r'), KeyModifiers::CONTROL));
        assert!(app.sort.sorted_columns().is_empty());
        assert_eq!(displayed_column(&app, 0), ["a", "b", "c"]);
    }

    #[test]
    fn control_shift_arrows_resize_the_selected_column_without_moving_selection() {
        let mut app = app("first,second\nabcdef,x\n", true, true);
        let selected = app.selected;
        let resize_modifiers = KeyModifiers::CONTROL | KeyModifiers::SHIFT;

        app.handle_event(modified_key(KeyCode::Left, resize_modifiers));
        assert_eq!(app.data.widths[0], 5);
        assert_eq!(app.data.column_starts[1], 7);
        assert_eq!(app.selected, selected);

        app.handle_event(modified_key(KeyCode::Right, resize_modifiers));
        assert_eq!(app.data.widths[0], 6);
        assert_eq!(app.data.column_starts[1], 8);
        assert_eq!(app.selected, selected);

        for _ in 0..10 {
            app.handle_event(modified_key(KeyCode::Left, resize_modifiers));
        }
        assert_eq!(app.data.widths[0], 1);
    }

    #[test]
    fn control_shift_arrows_do_nothing_without_a_selected_cell() {
        let mut app = app("header\n", true, true);
        let width = app.data.widths[0];

        app.handle_event(modified_key(
            KeyCode::Left,
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        ));

        assert_eq!(app.data.widths[0], width);
    }

    #[test]
    fn dragging_a_header_handle_resizes_its_column() {
        let mut app = app("first,second\nabcdef,x\n", true, true);

        app.handle_event(mouse(MouseEventKind::Down(MouseButton::Left), 7, 0));
        app.handle_event(mouse(MouseEventKind::Drag(MouseButton::Left), 4, 2));
        assert_eq!(app.data.widths[0], 3);
        assert_eq!(app.data.column_starts[1], 5);
        assert!(app.sort.sorted_columns().is_empty());

        app.handle_event(mouse(MouseEventKind::Up(MouseButton::Left), 4, 2));
        assert!(app.column_resize_drag.is_none());
    }

    #[test]
    fn clicking_a_visible_header_cell_sorts_while_clicking_a_handle_does_not() {
        let mut app = app("left,right\na,1\nb,2\n", true, true);

        app.handle_event(click(5, 0));
        assert!(app.sort.sorted_columns().is_empty());
        app.handle_event(mouse(MouseEventKind::Up(MouseButton::Left), 5, 0));

        app.column_offset = 4;
        app.handle_event(click(2, 0));
        assert_eq!(app.sort.sorted_columns(), [1]);
        assert_eq!(displayed_column(&app, 0), ["b", "a"]);

        app.handle_event(click(2, 0));
        assert_eq!(displayed_column(&app, 0), ["a", "b"]);
    }

    #[test]
    fn scrolling_header_only_accepts_clicks_while_visible() {
        let mut app = app("header\na\nb\n", true, false);
        app.row_offset = 1;

        app.handle_event(click(0, 0));

        assert!(app.sort.sorted_columns().is_empty());
    }

    #[test]
    fn empty_data_has_no_selection() {
        let app = app("header\n", true, true);
        assert_eq!(app.selected, None);
    }
}
