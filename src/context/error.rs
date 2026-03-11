//! Error types for context operations.

use crate::kubeconfig::KubeconfigError;

/// Errors that can occur during context operations.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum ContextError {
    /// No contexts were found in the kubeconfig file.
    #[error("no contexts found in kubeconfig")]
    NoContexts,

    /// The requested context does not exist.
    #[error("context {0:?} not found")]
    NotFound(String),

    /// An underlying kubeconfig operation failed.
    #[error(transparent)]
    Kubeconfig(#[from] KubeconfigError),

    /// A state file operation failed.
    #[error("failed to access state file")]
    State(#[source] std::io::Error),
}
