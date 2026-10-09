use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::{Duration, Instant},
};

use crossterm::event::KeyEvent;
use fancy_regex::RegexBuilder;
use ratatui_textarea::{CursorMove, TextArea};

pub(crate) const FILTER_DEBOUNCE: Duration = Duration::from_millis(175);
const REGEX_BACKTRACK_LIMIT: usize = 1_000_000;

type Rows = Arc<Vec<Vec<String>>>;

#[derive(Debug)]
struct FilterResult {
    generation: u64,
    patterns: Vec<String>,
    row_indices: Vec<usize>,
    errors: Vec<Option<String>>,
}

#[derive(Debug)]
pub(crate) struct FilterUpdate {
    pub(crate) row_indices: Vec<usize>,
}

#[derive(Debug)]
pub(crate) struct FilterState {
    visible: bool,
    editing: bool,
    // Editors and worker results use source column indices, independent of display order.
    editors: Vec<TextArea<'static>>,
    active_column: usize,
    pending_since: Option<Instant>,
    processing_patterns: Option<Vec<String>>,
    generation: u64,
    active_cancel: Option<Arc<AtomicBool>>,
    result_tx: Sender<FilterResult>,
    result_rx: Receiver<FilterResult>,
    errors: Vec<Option<String>>,
}

impl FilterState {
    pub(crate) fn new(column_count: usize) -> Self {
        let (result_tx, result_rx) = mpsc::channel();
        Self {
            visible: false,
            editing: false,
            editors: (0..column_count).map(|_| new_editor()).collect(),
            active_column: 0,
            pending_since: None,
            processing_patterns: None,
            generation: 0,
            active_cancel: None,
            result_tx,
            result_rx,
            errors: vec![None; column_count],
        }
    }

    pub(crate) fn is_visible(&self) -> bool {
        self.visible
    }

    pub(crate) fn is_processing(&self) -> bool {
        self.active_cancel.is_some()
    }

    pub(crate) fn is_editing(&self) -> bool {
        self.editing
    }

    pub(crate) fn active_column(&self) -> usize {
        self.active_column
    }

    pub(crate) fn activate_column(&mut self, column: usize) {
        if !self.editors.is_empty() {
            self.active_column = column.min(self.editors.len() - 1);
        }
    }

    pub(crate) fn focus_column(&mut self, column: usize) {
        self.activate_column(column);
        self.editing = !self.editors.is_empty();
    }

    pub(crate) fn editor_mut(&mut self, column: usize) -> Option<&mut TextArea<'static>> {
        self.editors.get_mut(column)
    }

    pub(crate) fn has_error(&self, column: usize) -> bool {
        self.errors.get(column).is_some_and(Option::is_some)
    }

    pub(crate) fn input(&mut self, key: KeyEvent, now: Instant) -> bool {
        if !self.editing {
            return false;
        }
        let Some(editor) = self.editors.get_mut(self.active_column) else {
            return false;
        };
        let modified = editor.input(key);
        if modified {
            self.pending_since = Some(now);
        }
        modified
    }

    pub(crate) fn move_to_line_start(&mut self) {
        if self.editing
            && let Some(editor) = self.editors.get_mut(self.active_column)
        {
            editor.move_cursor(CursorMove::Head);
        }
    }

    pub(crate) fn toggle(
        &mut self,
        rows: Rows,
        selected_column: Option<usize>,
    ) -> Option<FilterUpdate> {
        if self.visible {
            self.visible = false;
            self.editing = false;
            self.invalidate_active_work();
            self.processing_patterns = None;
            return Some(FilterUpdate {
                row_indices: all_row_indices(&rows),
            });
        }

        self.visible = true;
        if let Some(column) = selected_column {
            self.activate_column(column);
        }
        self.editing = !self.editors.is_empty();
        self.pending_since = None;
        self.start_current_patterns(rows)
    }

    pub(crate) fn finish_editing(&mut self, rows: Rows) -> Option<FilterUpdate> {
        if !self.visible || !self.editing {
            return None;
        }
        self.editing = false;
        self.pending_since = None;
        self.start_current_patterns(rows)
    }

    pub(crate) fn reset(&mut self, rows: &Rows) -> FilterUpdate {
        self.invalidate_active_work();
        for editor in &mut self.editors {
            *editor = new_editor();
        }
        self.errors.fill(None);
        self.processing_patterns = Some(vec![String::new(); self.editors.len()]);
        FilterUpdate {
            row_indices: all_row_indices(rows),
        }
    }

    pub(crate) fn tick(&mut self, rows: Rows, now: Instant) -> Option<FilterUpdate> {
        let debounce_elapsed = self
            .pending_since
            .is_some_and(|edited_at| now.saturating_duration_since(edited_at) >= FILTER_DEBOUNCE);
        let mut update = if self.visible && debounce_elapsed {
            self.pending_since = None;
            self.start_current_patterns(rows)
        } else {
            None
        };

        while let Ok(result) = self.result_rx.try_recv() {
            if self.visible && result.generation == self.generation {
                self.active_cancel = None;
                self.processing_patterns = Some(result.patterns);
                self.errors = result.errors;
                update = Some(FilterUpdate {
                    row_indices: result.row_indices,
                });
            }
        }
        update
    }

    fn start_current_patterns(&mut self, rows: Rows) -> Option<FilterUpdate> {
        let patterns = self.patterns();
        if self.processing_patterns.as_ref() == Some(&patterns) {
            return None;
        }

        self.invalidate_active_work();
        self.processing_patterns = Some(patterns.clone());
        self.errors.fill(None);
        if patterns.iter().all(String::is_empty) {
            return Some(FilterUpdate {
                row_indices: all_row_indices(&rows),
            });
        }

        let generation = self.generation;
        let cancel = Arc::new(AtomicBool::new(false));
        self.active_cancel = Some(Arc::clone(&cancel));
        let result_tx = self.result_tx.clone();
        thread::spawn(move || {
            if let Some(result) = filter_rows(rows, patterns, generation, &cancel) {
                let _ = result_tx.send(result);
            }
        });
        None
    }

    fn patterns(&self) -> Vec<String> {
        self.editors
            .iter()
            .map(|editor| editor.lines().first().cloned().unwrap_or_default())
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn pattern(&self, column: usize) -> Option<&str> {
        self.editors
            .get(column)
            .and_then(|editor| editor.lines().first())
            .map(String::as_str)
    }

    fn invalidate_active_work(&mut self) {
        if let Some(cancel) = self.active_cancel.take() {
            cancel.store(true, Ordering::Release);
        }
        self.generation = self.generation.wrapping_add(1);
        self.pending_since = None;
    }
}

impl Drop for FilterState {
    fn drop(&mut self) {
        if let Some(cancel) = self.active_cancel.take() {
            cancel.store(true, Ordering::Release);
        }
    }
}

fn new_editor() -> TextArea<'static> {
    let mut editor = TextArea::default();
    editor.set_cursor_line_style(ratatui::style::Style::default());
    editor.set_styled_placeholder("filter…");
    editor
}

fn all_row_indices(rows: &Rows) -> Vec<usize> {
    (0..rows.len()).collect()
}

fn filter_rows(
    rows: Rows,
    patterns: Vec<String>,
    generation: u64,
    cancel: &AtomicBool,
) -> Option<FilterResult> {
    if cancel.load(Ordering::Acquire) {
        return None;
    }

    let mut errors = vec![None; patterns.len()];
    let mut regexes = Vec::new();
    for (column, pattern) in patterns.iter().enumerate() {
        if pattern.is_empty() {
            continue;
        }
        let regex = RegexBuilder::new(pattern)
            .backtrack_limit(REGEX_BACKTRACK_LIMIT)
            .build();
        match regex {
            Ok(regex) => regexes.push((column, regex)),
            Err(error) => errors[column] = Some(error.to_string()),
        }
    }

    if errors.iter().any(Option::is_some) {
        return Some(FilterResult {
            generation,
            patterns,
            row_indices: Vec::new(),
            errors,
        });
    }

    let mut row_indices = Vec::new();
    for (row_index, row) in rows.iter().enumerate() {
        if cancel.load(Ordering::Acquire) {
            return None;
        }

        let mut matches = true;
        for (column, regex) in &regexes {
            if cancel.load(Ordering::Acquire) {
                return None;
            }
            let value = row.get(*column).map_or("", String::as_str);
            match regex.is_match(value) {
                Ok(true) => {}
                Ok(false) => {
                    matches = false;
                    break;
                }
                Err(error) => {
                    errors[*column] = Some(error.to_string());
                    return Some(FilterResult {
                        generation,
                        patterns,
                        row_indices: Vec::new(),
                        errors,
                    });
                }
            }
        }
        if matches {
            row_indices.push(row_index);
        }
    }

    Some(FilterResult {
        generation,
        patterns,
        row_indices,
        errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(rows: &[&[&str]]) -> Rows {
        Arc::new(
            rows.iter()
                .map(|row| row.iter().map(|cell| (*cell).to_owned()).collect())
                .collect(),
        )
    }

    #[test]
    fn filters_columns_with_fancy_regexes_and_ands_the_patterns() {
        let rows = rows(&[&["Ada", "London"], &["Grace", "Rome"], &["Alan", "London"]]);
        let result = filter_rows(
            rows,
            vec![r"(?<=A)d".into(), "London".into()],
            1,
            &AtomicBool::new(false),
        )
        .unwrap();

        assert_eq!(result.row_indices, [0]);
        assert!(result.errors.iter().all(Option::is_none));
    }

    #[test]
    fn missing_ragged_cells_are_treated_as_empty_strings() {
        let rows = rows(&[&["one"], &["two", "present"]]);
        let result = filter_rows(
            rows,
            vec![String::new(), "^$".into()],
            1,
            &AtomicBool::new(false),
        )
        .unwrap();

        assert_eq!(result.row_indices, [0]);
    }

    #[test]
    fn invalid_regexes_report_their_column_and_match_no_rows() {
        let result = filter_rows(
            rows(&[&["one"], &["two"]]),
            vec!["(".into()],
            1,
            &AtomicBool::new(false),
        )
        .unwrap();

        assert!(result.row_indices.is_empty());
        assert!(result.errors[0].is_some());
    }

    #[test]
    fn an_already_cancelled_filter_does_not_produce_a_result() {
        assert!(
            filter_rows(
                rows(&[&["one"]]),
                vec!["one".into()],
                1,
                &AtomicBool::new(true),
            )
            .is_none()
        );
    }

    #[test]
    fn starting_a_new_generation_cancels_the_previous_worker() {
        let rows = rows(&[&["one"], &["two"]]);
        let mut state = FilterState::new(1);
        state.visible = true;
        state.editors[0].insert_str("one");
        state.start_current_patterns(Arc::clone(&rows));
        let first_cancel = Arc::clone(state.active_cancel.as_ref().unwrap());

        state.editors[0] = TextArea::from(["two"]);
        state.start_current_patterns(rows);

        assert!(first_cancel.load(Ordering::Acquire));
        assert_eq!(
            state.processing_patterns.as_deref(),
            Some(&["two".into()][..])
        );
    }
}
