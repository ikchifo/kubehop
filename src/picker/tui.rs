//! Inline fuzzy picker using ratatui and crossterm.
//!
//! Renders a compact selection UI to stderr using `Viewport::Inline`,
//! keeping stdout free for machine-readable output.

use std::io::{Stderr, stderr};

use crossterm::cursor::SetCursorStyle;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};
use ratatui::{Terminal, TerminalOptions, Viewport};

use nucleo_matcher::{Config, Matcher};

use super::score::{ScoredItem, score_items_with_matcher};
use super::{PickerItem, PickerResult};

/// Maximum visible lines for the inline viewport (prompt + list rows).
const PICKER_HEIGHT: u16 = 15;

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
        enable_raw_mode()?;
        crossterm::execute!(stderr(), SetCursorStyle::BlinkingBlock)?;

        let backend = CrosstermBackend::new(stderr());
        let terminal = Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Inline(PICKER_HEIGHT),
            },
        )?;

        Ok(Self { terminal })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = crossterm::execute!(stderr(), SetCursorStyle::DefaultUserShape);
        let _ = disable_raw_mode();
        let _ = self.terminal.clear();
    }
}

/// Mutable state driving the picker event loop.
struct PickerState {
    query: String,
    scored: Vec<ScoredItem>,
    list_state: ListState,
    matcher: Matcher,
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
        if let Some(i) = self.list_state.selected() {
            if i > 0 {
                self.list_state.select(Some(i - 1));
            }
        }
    }

    fn move_down(&mut self) {
        if let Some(i) = self.list_state.selected() {
            if i + 1 < self.scored.len() {
                self.list_state.select(Some(i + 1));
            }
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
        terminal.draw(|frame| render(frame, items, &mut state))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

            match key.code {
                KeyCode::Esc => return Ok(PickerResult::Cancelled),
                KeyCode::Char('c') if ctrl => return Ok(PickerResult::Cancelled),
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
                KeyCode::Down => state.move_down(),
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
    }
}

/// Suspend the process (Ctrl+Z) by restoring the terminal and sending SIGTSTP.
///
/// On resume, raw mode and cursor style are re-established so the picker
/// can continue where it left off.
fn suspend(terminal: &mut Terminal<CrosstermBackend<Stderr>>) -> anyhow::Result<()> {
    let _ = crossterm::execute!(stderr(), SetCursorStyle::DefaultUserShape);
    disable_raw_mode()?;
    let _ = terminal.clear();

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

    let [list_area, status_area, input_area] = Layout::vertical([
        Constraint::Fill(1),
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

    frame.render_stateful_widget(list, list_area, &mut state.list_state);

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
