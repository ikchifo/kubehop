//! Inline fuzzy picker using ratatui and crossterm.
//!
//! Renders a compact selection UI to stderr using `Viewport::Inline`,
//! keeping stdout free for machine-readable output.

use std::io::{Stderr, stderr};

use crossterm::cursor::{MoveTo, SetCursorStyle};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{Clear, ClearType as CtClearType, disable_raw_mode, enable_raw_mode};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};
use ratatui::{Terminal, TerminalOptions, Viewport};

use nucleo_matcher::{Config, Matcher};

use crate::kubeconfig::ContextFields;

use super::score::{ScoredItem, score_items_with_matcher};
use super::{PickerItem, PickerResult};

/// Maximum visible lines for the inline viewport (list + preview + status + input).
const PICKER_HEIGHT: u16 = 16;

const PROMPT: &str = "> ";

/// Launch an inline fuzzy picker on stderr and return the user's selection.
///
/// The picker renders at the current cursor position using
/// `Viewport::Inline`, scores items with `nucleo-matcher`, and
/// accepts keyboard input for filtering and navigation.
///
/// Terminal state is restored via RAII (`TerminalGuard`) so cleanup
/// happens even on panic.
///
/// # Errors
///
/// Returns an error if terminal setup, rendering, or event reading fails.
pub fn pick_inline(items: &[PickerItem]) -> anyhow::Result<PickerResult> {
    let mut guard = TerminalGuard::new()?;
    run_picker_loop(&mut guard.terminal, items)
}

/// RAII guard that restores terminal state on drop.
struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stderr>>,
}

impl TerminalGuard {
    fn new() -> anyhow::Result<Self> {
        let terminal = setup_with_rollback(
            enable_raw_mode,
            || crossterm::execute!(stderr(), SetCursorStyle::BlinkingBlock),
            || {
                let backend = CrosstermBackend::new(stderr());
                Terminal::with_options(
                    backend,
                    TerminalOptions {
                        viewport: Viewport::Inline(PICKER_HEIGHT),
                    },
                )
            },
            rollback_terminal_setup,
        )?;

        Ok(Self { terminal })
    }
}

struct SetupRollback<F: FnOnce()> {
    rollback: Option<F>,
}

impl<F: FnOnce()> SetupRollback<F> {
    fn new(rollback: F) -> Self {
        Self {
            rollback: Some(rollback),
        }
    }

    fn disarm(&mut self) {
        self.rollback = None;
    }
}

impl<F: FnOnce()> Drop for SetupRollback<F> {
    fn drop(&mut self) {
        if let Some(rollback) = self.rollback.take() {
            rollback();
        }
    }
}

fn setup_with_rollback<T, E>(
    enable_raw: impl FnOnce() -> Result<(), E>,
    set_cursor: impl FnOnce() -> Result<(), E>,
    create_terminal: impl FnOnce() -> Result<T, E>,
    rollback: impl FnOnce(),
) -> Result<T, E> {
    enable_raw()?;
    let mut setup_rollback = SetupRollback::new(rollback);
    set_cursor()?;
    let terminal = create_terminal()?;
    setup_rollback.disarm();
    Ok(terminal)
}

fn rollback_terminal_setup() {
    let _ = crossterm::execute!(stderr(), SetCursorStyle::DefaultUserShape);
    let _ = disable_raw_mode();
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = cleanup_viewport(&mut self.terminal);
        let _ = disable_raw_mode();
    }
}

/// Move cursor to the viewport origin and clear downward, then restore
/// the default cursor shape. We avoid `terminal.clear()` because it
/// restores the cursor to its pre-clear position (the bottom input
/// line), leaving a blank gap above the shell prompt.
fn cleanup_viewport(terminal: &mut Terminal<CrosstermBackend<Stderr>>) -> std::io::Result<()> {
    let y = terminal.get_frame().area().y;
    crossterm::execute!(
        stderr(),
        MoveTo(0, y),
        Clear(CtClearType::FromCursorDown),
        SetCursorStyle::DefaultUserShape,
    )
}

/// Mutable state driving the picker event loop.
struct PickerState {
    query: String,
    scored: Vec<ScoredItem>,
    list_state: ListState,
    matcher: Matcher,
    /// Visible list rows from the last render, used for page step calculation.
    visible_rows: usize,
}

impl PickerState {
    fn new(items: &[PickerItem]) -> Self {
        let mut matcher = Matcher::new(Config::DEFAULT);
        let scored = score_items_with_matcher(items, "", &mut matcher);
        let mut list_state = ListState::default();
        if !scored.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            query: String::new(),
            scored,
            list_state,
            matcher,
            visible_rows: usize::from(PICKER_HEIGHT.saturating_sub(3)),
        }
    }

    fn update_scores(&mut self, items: &[PickerItem]) {
        self.scored = score_items_with_matcher(items, &self.query, &mut self.matcher);
        if self.scored.is_empty() {
            self.list_state.select(None);
        } else {
            self.list_state.select(Some(0));
        }
    }

    fn move_up(&mut self) {
        if let Some(i) = self.list_state.selected()
            && i > 0
        {
            self.list_state.select(Some(i - 1));
        }
    }

    fn move_down(&mut self) {
        if let Some(i) = self.list_state.selected()
            && i + 1 < self.scored.len()
        {
            self.list_state.select(Some(i + 1));
        }
    }

    fn page_up(&mut self, step: usize) {
        if let Some(i) = self.list_state.selected() {
            self.list_state.select(Some(i.saturating_sub(step)));
        }
    }

    fn page_down(&mut self, step: usize) {
        if let Some(i) = self.list_state.selected() {
            let last = self.scored.len().saturating_sub(1);
            self.list_state.select(Some((i + step).min(last)));
        }
    }

    fn move_first(&mut self) {
        if !self.scored.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    fn move_last(&mut self) {
        if !self.scored.is_empty() {
            self.list_state.select(Some(self.scored.len() - 1));
        }
    }

    fn selected_name<'a>(&self, items: &'a [PickerItem]) -> Option<&'a str> {
        let sel = self.list_state.selected()?;
        let scored = self.scored.get(sel)?;
        Some(&items[scored.index].name)
    }
}

fn run_picker_loop(
    terminal: &mut Terminal<CrosstermBackend<Stderr>>,
    items: &[PickerItem],
) -> anyhow::Result<PickerResult> {
    let mut state = PickerState::new(items);

    loop {
        terminal.draw(|f| render(f, items, &mut state))?;

        match event::read()? {
            Event::Resize(..) => {
                // Eagerly update internal buffers so the next draw() renders
                // at the correct dimensions. Ratatui (patched with PR #2355)
                // handles horizontal-shrink artifacts internally.
                terminal.autoresize()?;
            }
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

                match key.code {
                    KeyCode::Esc => return Ok(PickerResult::Cancelled),
                    KeyCode::Char('c') if ctrl => return Ok(PickerResult::Cancelled),
                    KeyCode::Char('l') if ctrl => {
                        terminal.clear()?;
                    }
                    KeyCode::Char('z') if ctrl => {
                        suspend(terminal)?;
                    }
                    KeyCode::Enter => {
                        return Ok(match state.selected_name(items) {
                            Some(name) => PickerResult::Selected(name.to_owned()),
                            None => PickerResult::Cancelled,
                        });
                    }
                    KeyCode::Up => state.move_up(),
                    KeyCode::Char('p') if ctrl => state.move_up(),
                    KeyCode::Down => state.move_down(),
                    KeyCode::Char('n') if ctrl => state.move_down(),
                    KeyCode::PageUp => state.page_up(state.visible_rows),
                    KeyCode::PageDown => state.page_down(state.visible_rows),
                    KeyCode::Home => state.move_first(),
                    KeyCode::End => state.move_last(),
                    KeyCode::Char('u') if ctrl => {
                        state.query.clear();
                        state.update_scores(items);
                    }
                    KeyCode::Char('w') if ctrl => {
                        let trimmed = state.query.trim_end();
                        let boundary = trimmed.rfind(' ').map_or(0, |pos| pos + 1);
                        state.query.truncate(boundary);
                        state.update_scores(items);
                    }
                    KeyCode::Backspace => {
                        state.query.pop();
                        state.update_scores(items);
                    }
                    KeyCode::Char(c) => {
                        state.query.push(c);
                        state.update_scores(items);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

/// Suspend the process (Ctrl+Z) by restoring the terminal and sending SIGTSTP.
///
/// On resume, raw mode and cursor style are re-established so the picker
/// can continue where it left off.
fn suspend(terminal: &mut Terminal<CrosstermBackend<Stderr>>) -> anyhow::Result<()> {
    cleanup_viewport(terminal)?;
    disable_raw_mode()?;

    #[cfg(unix)]
    {
        // SAFETY: `kill(0, SIGTSTP)` sends the signal to our own process
        // group. This is the standard mechanism for voluntary suspension
        // (equivalent to what the shell does on Ctrl+Z). No memory or
        // resource invariants are violated.
        unsafe {
            libc::kill(0, libc::SIGTSTP);
        }
    }

    // Re-enter raw mode after the shell foregrounds us.
    enable_raw_mode()?;
    crossterm::execute!(stderr(), SetCursorStyle::BlinkingBlock)?;
    Ok(())
}

fn render(frame: &mut ratatui::Frame, items: &[PickerItem], state: &mut PickerState) {
    let area = frame.area();

    let [list_area, preview_area, status_area, input_area] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(area);

    let highlight_style = Style::default()
        .fg(Color::Green)
        .add_modifier(Modifier::BOLD);

    let list_items: Vec<ListItem> = state
        .scored
        .iter()
        .map(|scored| {
            let item = &items[scored.index];
            let prefix = if item.is_current { "* " } else { "  " };
            let name = &item.name;

            let mut spans = vec![Span::raw(prefix)];

            if scored.indices.is_empty() {
                spans.push(Span::raw(name.as_str()));
            } else {
                build_highlighted_spans(name, &scored.indices, highlight_style, &mut spans);
            }

            ListItem::new(Line::from(spans))
        })
        .collect();

    let match_count = state.scored.len();
    let total = items.len();

    let list = List::new(list_items)
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("\u{25b8} ");

    state.visible_rows = usize::from(list_area.height);
    frame.render_stateful_widget(list, list_area, &mut state.list_state);

    render_preview(frame, preview_area, items, state);

    let status_text = format!("  [{match_count}/{total}]");
    let status = Paragraph::new(status_text).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(status, status_area);

    let input_text = format!("{PROMPT}{}", state.query);
    let input = Paragraph::new(input_text).style(Style::default().fg(Color::Yellow));
    frame.render_widget(input, input_area);

    #[allow(clippy::cast_possible_truncation)]
    let cursor_x = input_area.x + PROMPT.len() as u16 + state.query.len() as u16;
    frame.set_cursor_position((cursor_x, input_area.y));
}

fn render_preview(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    items: &[PickerItem],
    state: &PickerState,
) {
    let line = match selected_meta(items, state) {
        Some(meta) => {
            let ns = meta.namespace.as_deref().unwrap_or("-");
            let cluster = meta.cluster.as_deref().unwrap_or("-");
            let user = meta.user.as_deref().unwrap_or("-");
            Line::from(vec![
                Span::raw("  ns="),
                Span::raw(ns),
                Span::raw(" | cluster="),
                Span::raw(cluster),
                Span::raw(" | user="),
                Span::raw(user),
            ])
        }
        None => Line::default(),
    };
    let preview = Paragraph::new(line).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(preview, area);
}

fn selected_meta<'a>(items: &'a [PickerItem], state: &PickerState) -> Option<&'a ContextFields> {
    let sel = state.list_state.selected()?;
    let scored = state.scored.get(sel)?;
    items[scored.index].meta.as_ref()
}

/// Build spans for a name string with highlighted match positions.
///
/// Assumes `indices` contains sorted, deduplicated byte positions that
/// are valid for ASCII strings (kubernetes context names are DNS-compatible).
fn build_highlighted_spans<'a>(
    name: &'a str,
    indices: &[u32],
    style: Style,
    spans: &mut Vec<Span<'a>>,
) {
    let mut last = 0usize;

    for &idx in indices {
        let idx = idx as usize;
        if idx >= name.len() {
            continue;
        }
        if idx > last {
            spans.push(Span::raw(&name[last..idx]));
        }
        let end = idx + name[idx..].chars().next().map_or(1, char::len_utf8);
        spans.push(Span::styled(&name[idx..end], style));
        last = end;
    }

    if last < name.len() {
        spans.push(Span::raw(&name[last..]));
    }
}

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;

    use super::*;

    fn item(name: &str, is_current: bool, meta: Option<ContextFields>) -> PickerItem {
        PickerItem {
            name: name.to_owned(),
            is_current,
            meta,
        }
    }

    fn render_buffer(items: &[PickerItem], state: &mut PickerState) -> Buffer {
        let backend = TestBackend::new(60, 8);
        let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
        terminal
            .draw(|frame| render(frame, items, state))
            .expect("picker should render");
        terminal.backend().buffer().clone()
    }

    fn row(buffer: &Buffer, y: u16) -> String {
        (0..buffer.area.width)
            .map(|x| buffer[(x, y)].symbol())
            .collect::<String>()
            .trim_end()
            .to_owned()
    }

    #[test]
    fn render_shows_context_details_status_and_prompt() {
        let items = vec![
            item(
                "production",
                true,
                Some(ContextFields {
                    namespace: Some("payments".to_owned()),
                    cluster: Some("prod-eu".to_owned()),
                    user: Some("operator".to_owned()),
                }),
            ),
            item("staging", false, None),
        ];
        let mut state = PickerState::new(&items);

        let buffer = render_buffer(&items, &mut state);

        assert!(row(&buffer, 0).contains("production"));
        assert!(row(&buffer, 1).contains("staging"));
        assert_eq!(
            row(&buffer, 5),
            "  ns=payments | cluster=prod-eu | user=operator"
        );
        assert_eq!(row(&buffer, 6), "  [2/2]");
        assert_eq!(row(&buffer, 7), ">");
    }

    #[test]
    fn render_updates_preview_for_selected_item() {
        let items = vec![
            item(
                "production",
                true,
                Some(ContextFields {
                    namespace: Some("payments".to_owned()),
                    cluster: Some("prod-eu".to_owned()),
                    user: Some("operator".to_owned()),
                }),
            ),
            item(
                "staging",
                false,
                Some(ContextFields {
                    namespace: Some("preview".to_owned()),
                    cluster: Some("stage-us".to_owned()),
                    user: Some("developer".to_owned()),
                }),
            ),
        ];
        let mut state = PickerState::new(&items);
        state.move_down();

        let buffer = render_buffer(&items, &mut state);

        assert_eq!(
            row(&buffer, 5),
            "  ns=preview | cluster=stage-us | user=developer"
        );
    }

    #[test]
    fn navigation_stays_within_matching_items() {
        let items = vec![
            item("production", false, None),
            item("staging", false, None),
        ];
        let mut state = PickerState::new(&items);

        state.move_up();
        assert_eq!(state.list_state.selected(), Some(0));

        state.move_down();
        state.move_down();
        assert_eq!(state.list_state.selected(), Some(1));

        state.query = "missing".to_owned();
        state.update_scores(&items);
        assert_eq!(state.list_state.selected(), None);
    }

    #[test]
    fn terminal_setup_rolls_back_when_cursor_setup_fails() {
        let events = std::cell::RefCell::new(Vec::new());

        let result: Result<(), &str> = setup_with_rollback(
            || {
                events.borrow_mut().push("raw");
                Ok(())
            },
            || {
                events.borrow_mut().push("cursor");
                Err("cursor failed")
            },
            || {
                events.borrow_mut().push("terminal");
                Ok(())
            },
            || events.borrow_mut().push("rollback"),
        );

        assert_eq!(result, Err("cursor failed"));
        assert_eq!(*events.borrow(), ["raw", "cursor", "rollback"]);
    }

    #[test]
    fn terminal_setup_rolls_back_when_terminal_creation_fails() {
        let events = std::cell::RefCell::new(Vec::new());

        let result: Result<(), &str> = setup_with_rollback(
            || {
                events.borrow_mut().push("raw");
                Ok(())
            },
            || {
                events.borrow_mut().push("cursor");
                Ok(())
            },
            || {
                events.borrow_mut().push("terminal");
                Err("terminal failed")
            },
            || events.borrow_mut().push("rollback"),
        );

        assert_eq!(result, Err("terminal failed"));
        assert_eq!(*events.borrow(), ["raw", "cursor", "terminal", "rollback"]);
    }
}
