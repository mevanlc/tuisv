use std::{
    io,
    io::stdout,
    panic,
    sync::Arc,
    time::{Duration, Instant},
};

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    execute,
};
use ratatui::DefaultTerminal;

use crate::{
    data::TableData,
    filter::{FilterState, FilterUpdate},
    sort::SortState,
    ui,
};

const VERTICAL_SCROLL_AMOUNT: usize = 3;
const HORIZONTAL_SCROLL_AMOUNT: usize = 4;
const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(25);

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
    pub data: TableData,
    alternate_data: Option<TableData>,
    pub sticky_header: bool,
    pub selected: Option<CellPosition>,
    pub row_offset: usize,
    pub column_offset: usize,
    sort: SortState,
    pub(crate) filter: FilterState,
    help_visible: bool,
    help_scroll: usize,
    column_resize_drag: Option<ColumnResizeDrag>,
    viewport_width: usize,
    viewport_height: usize,
    quit: bool,
}

impl App {
    pub fn new(data: TableData, sticky_header: bool) -> Self {
        let selected = (!data.rows.is_empty() && data.column_count() > 0)
            .then_some(CellPosition { row: 0, column: 0 });
        let sort = SortState::new(data.rows.len());
        let filter = FilterState::new(data.column_count());
        Self {
            data,
            alternate_data: None,
            sticky_header,
            selected,
            row_offset: 0,
            column_offset: 0,
            sort,
            filter,
            help_visible: false,
            help_scroll: 0,
            column_resize_drag: None,
            viewport_width: 0,
            viewport_height: 0,
            quit: false,
        }
    }

    pub fn set_viewport(&mut self, width: u16, height: u16) {
        self.viewport_width = usize::from(width);
        self.viewport_height = usize::from(height);
        self.help_scroll = self.help_scroll.min(ui::help_max_scroll(
            self.viewport_width,
            self.viewport_height,
        ));
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

    pub fn tick(&mut self) {
        self.tick_at(Instant::now());
    }

    fn tick_at(&mut self, now: Instant) {
        if let Some(update) = self.filter.tick(Arc::clone(&self.data.rows), now) {
            self.apply_filter_update(update);
        }
    }

    pub fn scrollable_height(&self) -> usize {
        let fixed_rows = usize::from(self.filter.is_visible())
            + usize::from(self.sticky_header && self.data.header.is_some());
        self.viewport_height.saturating_sub(fixed_rows)
    }

    pub fn scrollable_row_count(&self) -> usize {
        self.sort.row_count() + usize::from(!self.sticky_header && self.data.header.is_some())
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

    pub(crate) fn help_visible(&self) -> bool {
        self.help_visible
    }

    pub(crate) fn help_scroll(&self) -> usize {
        self.help_scroll
    }

    pub(crate) fn clamp_help_scroll(&mut self, maximum: usize) {
        self.help_scroll = self.help_scroll.min(maximum);
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return;
        }

        if matches!(key.code, KeyCode::Char('c')) && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.quit = true;
            return;
        }

        if self.help_visible {
            match key.code {
                KeyCode::Esc | KeyCode::Char('?') => self.help_visible = false,
                KeyCode::Up | KeyCode::Left => self.scroll_help(-1),
                KeyCode::Down | KeyCode::Right => self.scroll_help(1),
                _ => {}
            }
            return;
        }

        if key.code == KeyCode::Esc {
            self.go_back();
            return;
        }

        match key.code {
            KeyCode::Char('f' | 'F') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.toggle_filter();
                return;
            }
            KeyCode::Char('s' | 'S') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(selected) = self.selected {
                    self.sort.sort_by_column(&self.data, selected.column);
                }
                return;
            }
            KeyCode::Char('r' | 'R') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.reset_sort_and_filters();
                return;
            }
            KeyCode::Char('t' | 'T') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.toggle_transpose();
                return;
            }
            KeyCode::Left if key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT) => {
                self.resize_selected_column(-1);
                return;
            }
            KeyCode::Right if key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT) => {
                self.resize_selected_column(1);
                return;
            }
            _ => {}
        }

        if self.filter.is_editing() {
            match key.code {
                KeyCode::Char('a' | 'A') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.filter.move_to_line_start();
                }
                KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    self.move_filter_focus(-1);
                }
                KeyCode::Tab => self.move_filter_focus(1),
                KeyCode::BackTab => self.move_filter_focus(-1),
                KeyCode::Enter | KeyCode::Char('\n' | '\r') => self.finish_filter_editing(),
                KeyCode::Char('m' | 'M') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.finish_filter_editing();
                }
                _ => {
                    self.filter.input(key, Instant::now());
                }
            }
            return;
        }

        if key.code == KeyCode::Char('?') {
            self.help_visible = true;
            self.help_scroll = 0;
            return;
        }

        if self.filter.is_visible() && matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
            let backwards =
                key.code == KeyCode::BackTab || key.modifiers.contains(KeyModifiers::SHIFT);
            self.move_filter_focus(if backwards { -1 } else { 1 });
            return;
        }

        match key.code {
            KeyCode::Char('q' | 'Q') => self.quit = true,
            KeyCode::Up => self.move_selection(-1, 0),
            KeyCode::Down => self.move_selection(1, 0),
            KeyCode::Left => self.move_selection(0, -1),
            KeyCode::Right => self.move_selection(0, 1),
            _ => {}
        }
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) {
        if self.help_visible {
            return;
        }
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if self.activate_filter_at(mouse.column, mouse.row) {
                    self.column_resize_drag = None;
                } else if let Some(column) = self.resize_handle_at(mouse.column, mouse.row) {
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
            self.sort.sort_by_column(&self.data, column);
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
        let header_screen_row = u16::from(self.filter.is_visible());
        (self.data.header.is_some()
            && (self.sticky_header || self.row_offset == 0)
            && screen_row == header_screen_row)
            .then(|| {
                self.column_offset
                    .saturating_add(usize::from(screen_column))
            })
    }

    fn activate_filter_at(&mut self, screen_column: u16, screen_row: u16) -> bool {
        if !self.filter.is_visible() || screen_row != 0 {
            return false;
        }
        let content_column = self
            .column_offset
            .saturating_add(usize::from(screen_column));
        let Some(column) = self
            .data
            .column_starts
            .iter()
            .zip(&self.data.widths)
            .position(|(start, width)| {
                content_column >= *start && content_column < start.saturating_add(*width)
            })
        else {
            return false;
        };
        self.focus_filter_column(column);
        true
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
            .min(self.sort.row_count().saturating_sub(1));
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

    fn toggle_filter(&mut self) {
        let selected_column = self.selected.map(|selected| selected.column);
        let update = self
            .filter
            .toggle(Arc::clone(&self.data.rows), selected_column);
        if let Some(update) = update {
            self.apply_filter_update(update);
        }
        if self.filter.is_visible() {
            self.activate_filter_column(self.filter.active_column());
        }
    }

    fn reset_sort_and_filters(&mut self) {
        self.sort.reset();
        let update = self.filter.reset(&self.data.rows);
        self.apply_filter_update(update);
    }

    fn toggle_transpose(&mut self) {
        let alternate = self
            .alternate_data
            .get_or_insert_with(|| self.data.transpose());
        std::mem::swap(&mut self.data, alternate);

        self.sort = SortState::new(self.data.rows.len());
        // Dropping the old state cancels its worker and discards its result channel.
        self.filter = FilterState::new(self.data.column_count());
        self.selected = (!self.data.rows.is_empty() && self.data.column_count() > 0)
            .then_some(CellPosition { row: 0, column: 0 });
        self.row_offset = 0;
        self.column_offset = 0;
        self.column_resize_drag = None;
    }

    fn go_back(&mut self) {
        if self.filter.is_visible() {
            self.toggle_filter();
        } else if self.sort.is_active() {
            self.sort.reset();
        } else {
            self.quit = true;
        }
    }

    fn scroll_help(&mut self, delta: isize) {
        let maximum = ui::help_max_scroll(self.viewport_width, self.viewport_height);
        self.help_scroll = self.help_scroll.saturating_add_signed(delta).min(maximum);
    }

    fn finish_filter_editing(&mut self) {
        if let Some(update) = self.filter.finish_editing(Arc::clone(&self.data.rows)) {
            self.apply_filter_update(update);
        }
    }

    fn move_filter_focus(&mut self, delta: isize) {
        self.filter.move_active_column(delta);
        self.focus_filter_column(self.filter.active_column());
    }

    fn focus_filter_column(&mut self, column: usize) {
        self.filter.focus_column(column);
        self.activate_filter_column(column);
    }

    fn activate_filter_column(&mut self, column: usize) {
        self.filter.activate_column(column);
        if let Some(selected) = &mut self.selected {
            selected.column = column;
        }
        self.ensure_column_visible(column);
    }

    fn apply_filter_update(&mut self, update: FilterUpdate) {
        self.sort.replace_rows(update.row_indices, &self.data);
        self.reconcile_selection();
        self.ensure_selection_visible();
        self.clamp_offsets();
    }

    fn reconcile_selection(&mut self) {
        if self.sort.row_count() == 0 || self.data.column_count() == 0 {
            self.selected = None;
            return;
        }
        let row = self
            .selected
            .map_or(0, |selected| selected.row.min(self.sort.row_count() - 1));
        let column = self
            .selected
            .map_or(self.filter.active_column(), |selected| {
                selected.column.min(self.data.column_count() - 1)
            });
        self.selected = Some(CellPosition { row, column });
    }

    fn ensure_column_visible(&mut self, column: usize) {
        let Some((&column_start, &column_width)) = self
            .data
            .column_starts
            .get(column)
            .zip(self.data.widths.get(column))
        else {
            return;
        };
        let column_end = column_start.saturating_add(column_width);
        if column_start < self.column_offset || column_width > self.viewport_width {
            self.column_offset = column_start;
        } else if self.viewport_width > 0 && column_end > self.column_offset + self.viewport_width {
            self.column_offset = column_end - self.viewport_width;
        }
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
            app.tick();
            self.terminal.draw(|frame| ui::render(frame, app))?;
            if event::poll(EVENT_POLL_INTERVAL)? {
                app.handle_event(event::read()?);
            }
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
            TableData::from_reader(csv.as_bytes(), has_header).unwrap(),
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
        (0..app.sort.row_count())
            .map(|row| app.displayed_row(row).unwrap()[column].clone())
            .collect()
    }

    fn send_text(app: &mut App, text: &str) {
        for character in text.chars() {
            app.handle_event(key(KeyCode::Char(character)));
        }
    }

    fn wait_for_filtered_rows(app: &mut App, expected: usize) {
        app.tick_at(Instant::now() + crate::filter::FILTER_DEBOUNCE + Duration::from_millis(5));
        wait_for_filter_worker(app, expected);
    }

    fn wait_for_filter_worker(app: &mut App, expected: usize) {
        let deadline = Instant::now() + Duration::from_secs(1);
        while app.sort.row_count() != expected || app.filter.is_processing() {
            assert!(Instant::now() < deadline, "filter worker did not settle");
            app.tick();
            std::thread::sleep(Duration::from_millis(1));
        }
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
    fn control_t_restores_ragged_data_and_remembers_each_orientations_widths() {
        let mut app = app(include_str!("../samples/ragged.csv"), true, true);
        app.data.set_column_width(1, 1);
        let original = app.data.clone();
        app.set_viewport(3, 2);
        app.handle_event(key(KeyCode::Right));
        app.handle_event(key(KeyCode::Down));
        assert!(app.row_offset > 0);
        assert!(app.column_offset > 0);
        let handle = app.data.widths[0] + 1 - app.column_offset;
        app.handle_event(mouse(
            MouseEventKind::Down(MouseButton::Left),
            u16::try_from(handle).unwrap(),
            0,
        ));
        assert!(app.column_resize_drag.is_some());

        app.handle_event(modified_key(KeyCode::Char('t'), KeyModifiers::CONTROL));

        assert_eq!(app.selected, Some(CellPosition { row: 0, column: 0 }));
        assert_eq!(app.row_offset, 0);
        assert_eq!(app.column_offset, 0);
        assert!(app.column_resize_drag.is_none());
        app.data.set_column_width(0, 2);
        let transposed = app.data.clone();

        app.handle_event(modified_key(KeyCode::Char('T'), KeyModifiers::CONTROL));
        assert_eq!(app.data, original);
        assert!(Arc::ptr_eq(&app.data.rows, &original.rows));

        app.handle_event(modified_key(KeyCode::Char('t'), KeyModifiers::CONTROL));
        assert_eq!(app.data, transposed);
        assert!(Arc::ptr_eq(&app.data.rows, &transposed.rows));
    }

    #[test]
    fn control_t_uses_full_file_order_and_discards_active_filter_work() {
        let mut app = app(include_str!("../samples/people.csv"), true, true);
        app.handle_event(key(KeyCode::Right));
        app.handle_event(modified_key(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert_eq!(displayed_column(&app, 0), ["Bob", "Ada", "Grace"]);
        app.handle_event(modified_key(KeyCode::Char('f'), KeyModifiers::CONTROL));
        send_text(&mut app, "^37$");
        wait_for_filtered_rows(&mut app, 1);
        assert_eq!(displayed_column(&app, 0), ["Ada"]);

        send_text(&mut app, "x");
        let original_rows = Arc::clone(&app.data.rows);
        app.handle_event(key(KeyCode::Enter));
        assert!(app.filter.is_processing());
        app.handle_event(key(KeyCode::Tab));
        assert!(app.filter.is_editing());

        app.handle_event(modified_key(KeyCode::Char('t'), KeyModifiers::CONTROL));

        assert_eq!(
            app.data.header.as_deref(),
            Some(&["name".into(), "Ada".into(), "Bob".into(), "Grace".into()][..])
        );
        assert_eq!(displayed_column(&app, 0), ["age", "city"]);
        assert!(app.sort.sorted_columns().is_empty());
        assert!(!app.filter.is_visible());
        assert!(!app.filter.is_editing());
        assert!(!app.filter.is_processing());
        assert_eq!(app.filter.pattern(3), Some(""));
        assert_eq!(app.filter.pattern(4), None);

        // Wait for the old worker to release the source rows before ticking the new state.
        let deadline = Instant::now() + Duration::from_secs(1);
        while Arc::strong_count(&original_rows) > 2 {
            assert!(
                Instant::now() < deadline,
                "cancelled filter worker did not finish"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        app.tick_at(Instant::now() + Duration::from_secs(1));
        assert_eq!(displayed_column(&app, 0), ["age", "city"]);

        app.handle_event(modified_key(KeyCode::Char('f'), KeyModifiers::CONTROL));
        send_text(&mut app, "^age$");
        app.handle_event(key(KeyCode::Enter));
        wait_for_filter_worker(&mut app, 1);
        assert_eq!(displayed_column(&app, 0), ["age"]);

        app.handle_event(modified_key(KeyCode::Char('t'), KeyModifiers::CONTROL));
        assert_eq!(displayed_column(&app, 0), ["Ada", "Bob", "Grace"]);
        assert!(!app.filter.is_visible());
        assert_eq!(app.filter.pattern(0), Some(""));
    }

    #[test]
    fn control_t_reconciles_selection_for_empty_and_header_only_orientations() {
        let first_cell = Some(CellPosition { row: 0, column: 0 });
        for (csv, has_header, transposed_rows, selected) in [
            ("", true, 0, None),
            ("", false, 0, None),
            ("name,age\n", true, 1, first_cell),
            ("name\nAda\nBob\n", true, 0, None),
            ("Ada\nBob\n", false, 1, first_cell),
        ] {
            let mut app = app(csv, has_header, has_header);
            let original = app.data.clone();
            let original_selection = app.selected;

            app.handle_event(modified_key(KeyCode::Char('t'), KeyModifiers::CONTROL));
            assert_eq!(app.sort.row_count(), transposed_rows, "{csv:?}");
            assert_eq!(app.selected, selected, "{csv:?}");
            app.handle_event(key(KeyCode::Down));
            app.handle_event(key(KeyCode::Right));

            app.handle_event(modified_key(KeyCode::Char('t'), KeyModifiers::CONTROL));
            assert_eq!(app.data, original, "{csv:?}");
            assert_eq!(app.selected, original_selection, "{csv:?}");
        }
    }

    #[test]
    fn reset_and_escape_keep_the_transposed_orientation() {
        let mut app = app("name,age,city\nAda,37,London\nBob,42,Rome\n", true, true);
        app.handle_event(modified_key(KeyCode::Char('t'), KeyModifiers::CONTROL));
        let transposed = app.data.clone();
        app.handle_event(modified_key(KeyCode::Char('s'), KeyModifiers::CONTROL));
        app.handle_event(modified_key(KeyCode::Char('f'), KeyModifiers::CONTROL));
        send_text(&mut app, "^age$");
        wait_for_filtered_rows(&mut app, 1);

        app.handle_event(modified_key(KeyCode::Char('r'), KeyModifiers::CONTROL));
        assert_eq!(app.data, transposed);
        assert_eq!(displayed_column(&app, 0), ["age", "city"]);
        assert!(!app.sort.is_active());
        assert_eq!(app.filter.pattern(0), Some(""));

        app.handle_event(modified_key(KeyCode::Char('s'), KeyModifiers::CONTROL));
        app.handle_event(key(KeyCode::Esc));
        assert!(!app.filter.is_visible());
        assert!(app.sort.is_active());
        assert!(!app.should_quit());
        app.handle_event(key(KeyCode::Esc));
        assert!(!app.sort.is_active());
        assert!(!app.should_quit());
        assert_eq!(app.data, transposed);
        app.handle_event(key(KeyCode::Esc));
        assert!(app.should_quit());
        assert_eq!(app.data, transposed);
    }

    #[test]
    fn control_f_filters_with_fancy_regexes_across_columns_and_remembers_values() {
        let mut app = app(
            "name,city\nAda,London\nGrace,Rome\nAlan,London\n",
            true,
            true,
        );
        app.set_viewport(30, 5);

        app.handle_event(modified_key(KeyCode::Char('f'), KeyModifiers::CONTROL));
        assert!(app.filter.is_visible());
        assert_eq!(app.scrollable_height(), 3);
        send_text(&mut app, r"(?<=A)d");
        app.handle_event(key(KeyCode::Tab));
        send_text(&mut app, "London");
        assert_eq!(app.selected.unwrap().column, 1);

        wait_for_filtered_rows(&mut app, 1);
        assert_eq!(displayed_column(&app, 0), ["Ada"]);

        app.handle_event(modified_key(KeyCode::Char('f'), KeyModifiers::CONTROL));
        assert!(!app.filter.is_visible());
        assert_eq!(displayed_column(&app, 0), ["Ada", "Grace", "Alan"]);

        app.handle_event(modified_key(KeyCode::Char('f'), KeyModifiers::CONTROL));
        wait_for_filtered_rows(&mut app, 1);
        assert_eq!(displayed_column(&app, 0), ["Ada"]);
    }

    #[test]
    fn q_edits_a_visible_filter_instead_of_quitting() {
        let mut app = app("value\nq\nx\n", true, true);
        app.handle_event(modified_key(KeyCode::Char('f'), KeyModifiers::CONTROL));

        app.handle_event(key(KeyCode::Char('q')));
        wait_for_filtered_rows(&mut app, 1);

        assert!(!app.should_quit());
        assert_eq!(displayed_column(&app, 0), ["q"]);
    }

    #[test]
    fn enter_commits_filter_editing_and_releases_plain_keys() {
        let mut app = app("value\na\nb\n", true, true);
        app.set_viewport(30, 6);
        app.handle_event(modified_key(KeyCode::Char('f'), KeyModifiers::CONTROL));
        send_text(&mut app, "^a$");

        app.handle_event(key(KeyCode::Enter));
        assert!(!app.filter.is_editing());
        wait_for_filter_worker(&mut app, 1);
        assert_eq!(displayed_column(&app, 0), ["a"]);

        app.handle_event(key(KeyCode::Char('?')));
        assert!(app.help_visible());
        app.handle_event(key(KeyCode::Esc));
        assert!(!app.help_visible());
        assert!(app.filter.is_visible());
    }

    #[test]
    fn question_mark_is_filter_text_while_an_editor_has_focus() {
        let mut app = app("value\na\n", true, true);
        app.handle_event(modified_key(KeyCode::Char('f'), KeyModifiers::CONTROL));

        app.handle_event(key(KeyCode::Char('?')));

        assert!(!app.help_visible());
        assert_eq!(app.filter.pattern(0), Some("?"));
    }

    #[test]
    fn control_a_moves_to_the_start_of_the_filter_line() {
        let mut app = app("value\na\n", true, true);
        app.handle_event(modified_key(KeyCode::Char('f'), KeyModifiers::CONTROL));
        send_text(&mut app, "abc");

        app.handle_event(modified_key(KeyCode::Char('a'), KeyModifiers::CONTROL));
        send_text(&mut app, "z");

        assert_eq!(app.filter.pattern(0), Some("zabc"));
    }

    #[test]
    fn escape_closes_filter_then_resets_sort_then_exits() {
        let mut app = app("value\na\nb\n", true, true);
        app.handle_event(modified_key(KeyCode::Char('s'), KeyModifiers::CONTROL));
        app.handle_event(modified_key(KeyCode::Char('f'), KeyModifiers::CONTROL));

        app.handle_event(key(KeyCode::Esc));
        assert!(!app.filter.is_visible());
        assert!(app.sort.is_active());
        assert!(!app.should_quit());

        app.handle_event(key(KeyCode::Esc));
        assert!(!app.sort.is_active());
        assert_eq!(displayed_column(&app, 0), ["a", "b"]);
        assert!(!app.should_quit());

        app.handle_event(key(KeyCode::Esc));
        assert!(app.should_quit());
    }

    #[test]
    fn help_is_modal_and_arrow_scrollable() {
        let mut app = app("value\na\nb\n", true, true);
        app.set_viewport(40, 8);
        let selected = app.selected;

        app.handle_event(key(KeyCode::Char('?')));
        assert!(app.help_visible());
        let data = app.data.clone();
        app.handle_event(modified_key(KeyCode::Char('t'), KeyModifiers::CONTROL));
        assert_eq!(app.data, data);
        assert!(app.alternate_data.is_none());
        app.handle_event(key(KeyCode::Down));
        assert_eq!(app.help_scroll(), 1);
        assert_eq!(app.selected, selected);
        app.handle_event(key(KeyCode::Up));
        assert_eq!(app.help_scroll(), 0);

        app.handle_event(key(KeyCode::Char('?')));
        assert!(!app.help_visible());
        assert!(!app.should_quit());
    }

    #[test]
    fn control_r_clears_sort_and_filters_and_rejects_cancelled_results() {
        let mut app = app("value\na\nc\nb\n", true, true);
        app.handle_event(modified_key(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert_eq!(displayed_column(&app, 0), ["c", "b", "a"]);
        app.handle_event(modified_key(KeyCode::Char('f'), KeyModifiers::CONTROL));
        send_text(&mut app, "^a$");
        app.tick_at(Instant::now() + crate::filter::FILTER_DEBOUNCE + Duration::from_millis(5));

        app.handle_event(modified_key(KeyCode::Char('r'), KeyModifiers::CONTROL));
        assert!(app.sort.sorted_columns().is_empty());
        assert_eq!(displayed_column(&app, 0), ["a", "c", "b"]);

        let deadline = Instant::now() + Duration::from_millis(50);
        while Instant::now() < deadline {
            app.tick();
            std::thread::yield_now();
        }
        assert_eq!(displayed_column(&app, 0), ["a", "c", "b"]);

        app.handle_event(modified_key(KeyCode::Char('f'), KeyModifiers::CONTROL));
        app.handle_event(modified_key(KeyCode::Char('f'), KeyModifiers::CONTROL));
        assert_eq!(displayed_column(&app, 0), ["a", "c", "b"]);
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
    fn clicking_a_secondary_sort_promotes_it_before_the_next_click_toggles_it() {
        let mut app = app("first,second\nb,x\na,y\nc,x\n", true, true);
        app.set_viewport(20, 5);

        app.handle_event(click(0, 0));
        app.handle_event(click(7, 0));
        assert_eq!(app.sort.sorted_columns(), [1, 0]);
        assert_eq!(app.sort_indicator(0), Some('▽'));
        assert_eq!(displayed_column(&app, 0), ["a", "c", "b"]);

        app.handle_event(click(0, 0));
        assert_eq!(app.sort.sorted_columns(), [0, 1]);
        assert_eq!(app.sort_indicator(0), Some('▼'));
        assert_eq!(app.sort_indicator(1), Some('▽'));
        assert_eq!(displayed_column(&app, 0), ["c", "b", "a"]);

        app.handle_event(click(0, 0));
        assert_eq!(app.sort.sorted_columns(), [0, 1]);
        assert_eq!(app.sort_indicator(0), Some('▲'));
        assert_eq!(displayed_column(&app, 0), ["a", "b", "c"]);
    }

    #[test]
    fn filter_row_mouse_clicks_focus_fields_and_shift_header_hit_testing_down() {
        let mut app = app("left,right\na,1\nb,2\n", true, true);
        app.set_viewport(20, 4);
        app.handle_event(modified_key(KeyCode::Char('f'), KeyModifiers::CONTROL));

        app.handle_event(click(7, 0));
        assert_eq!(app.filter.active_column(), 1);
        assert!(app.sort.sorted_columns().is_empty());

        app.handle_event(click(7, 1));
        assert_eq!(app.sort.sorted_columns(), [1]);
        assert_eq!(displayed_column(&app, 0), ["b", "a"]);
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
