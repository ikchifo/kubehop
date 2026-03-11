//! Interactive fuzzy picker for context selection.

pub mod fzf;
pub mod recency;
mod score;
mod tui;

pub use score::{ScoredItem, score_items};
pub use tui::pick_inline;

use crate::kubeconfig::ContextFields;

/// A single item in the picker list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerItem {
    /// Display name shown in the picker.
    pub name: String,
    /// Whether this item represents the currently active context.
    pub is_current: bool,
    /// Rich metadata for preview (cluster, namespace, user).
    pub meta: Option<ContextFields>,
}

impl From<crate::context::list::ContextListItem> for PickerItem {
    fn from(item: crate::context::list::ContextListItem) -> Self {
        Self {
            name: item.name,
            is_current: item.is_current,
            meta: item.context,
        }
    }
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
