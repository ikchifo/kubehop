//! Interactive fuzzy picker for context selection.

pub mod fzf;
mod score;
mod tui;

pub use score::{ScoredItem, score_items};
pub use tui::pick_inline;

/// Optional metadata attached to a picker item for rich preview.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PickerMeta {
    pub namespace: Option<String>,
    pub cluster: Option<String>,
    pub user: Option<String>,
}

/// A single item in the picker list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerItem {
    /// Display name shown in the picker.
    pub name: String,
    /// Whether this item represents the currently active context.
    pub is_current: bool,
    /// Rich metadata for preview (cluster, namespace, user).
    pub meta: Option<PickerMeta>,
}

impl From<crate::context::list::ContextListItem> for PickerItem {
    fn from(item: crate::context::list::ContextListItem) -> Self {
        let meta = if item.namespace.is_some() || item.cluster.is_some() || item.user.is_some() {
            Some(PickerMeta {
                namespace: item.namespace,
                cluster: item.cluster,
                user: item.user,
            })
        } else {
            None
        };
        Self {
            name: item.name,
            is_current: item.is_current,
            meta,
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
