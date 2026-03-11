//! Error types for namespace operations.

use crate::context::error::ContextError;
use crate::kubeconfig::KubeconfigError;

/// Errors that can occur during namespace operations.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum NamespaceError {
    /// No current context is set in the kubeconfig.
    #[error("no current context set")]
    NoCurrentContext,

    /// The requested namespace does not exist on the cluster.
    #[error("namespace {0:?} not found on cluster")]
    NotFound(String),

    /// Listing namespaces from the cluster failed.
    #[error("failed to list namespaces from cluster")]
    ListFailed(String),

    /// kubectl binary was not found in PATH.
    #[error("kubectl not found in PATH")]
    KubectlNotFound,

    /// An underlying kubeconfig operation failed.
    #[error(transparent)]
    Kubeconfig(#[from] KubeconfigError),

    /// A state file operation failed.
    #[error("failed to access state file")]
    State(#[source] std::io::Error),
}

impl NamespaceError {
    pub(super) fn from_context_err(err: ContextError) -> Self {
        match err {
            ContextError::Kubeconfig(e) => Self::Kubeconfig(e),
            ContextError::State(e) => Self::State(e),
            other => Self::ListFailed(other.to_string()),
        }
    }
}
