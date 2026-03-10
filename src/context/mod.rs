// Rust guideline compliant 2026-02-21
//! Context operations: list, switch, mutate, state.

pub mod error;
pub mod list;
pub mod current;
pub mod state;
pub mod switch;
pub mod mutate;
pub(crate) mod yaml_helpers;

pub use error::ContextError;
