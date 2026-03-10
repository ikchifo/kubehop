// Rust guideline compliant 2026-02-21
//! Interactive fuzzy picker for context selection.

mod score;
mod tui;
pub mod fzf;

pub use score::{ScoredItem, score_items};
pub use tui::pick_inline;

/// A single item in the picker list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerItem {
    /// Display name shown in the picker.
    pub name: String,
    /// Whether this item represents the currently active context.
    pub is_current: bool,
}

/// Outcome of the interactive picker.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PickerResult {
    /// The user selected a context by name.
    Selected(String),
    /// The user cancelled without selecting.
    Cancelled,
}
