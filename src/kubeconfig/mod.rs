// Rust guideline compliant 2026-02-21
//! Kubeconfig file parsing and loading.

pub mod error;
pub mod load;
pub mod model;

pub use error::KubeconfigError;
pub use model::{ContextEntry, ContextFields, KubeConfigView};
