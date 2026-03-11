//! Namespace operations for kubens behavior.

pub mod current;
pub mod error;
pub mod list;
pub mod state;
pub mod switch;

/// The implicit namespace when none is configured on a context.
pub(crate) const DEFAULT_NAMESPACE: &str = "default";
