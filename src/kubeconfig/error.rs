//! Error types for kubeconfig operations.

use std::path::PathBuf;

/// Errors that can occur during kubeconfig parsing and loading.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum KubeconfigError {
    /// YAML content could not be parsed.
    #[error("failed to parse kubeconfig")]
    Parse(#[source] serde_yaml::Error),

    /// File could not be read from disk.
    #[error("failed to read kubeconfig: {}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// File could not be written to disk.
    #[error("failed to write kubeconfig: {}", path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// A write target could not be uniquely resolved across
    /// multiple kubeconfig files.
    #[error("context write target is ambiguous across multiple kubeconfig files")]
    AmbiguousWrite,
}

impl KubeconfigError {
    /// Returns `true` if the underlying I/O error is `NotFound`.
    #[must_use]
    pub fn is_not_found(&self) -> bool {
        matches!(
            self,
            Self::Read { source, .. } if source.kind() == std::io::ErrorKind::NotFound
        )
    }
}
