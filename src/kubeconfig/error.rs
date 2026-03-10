// Rust guideline compliant 2026-02-21
//! Error types for kubeconfig operations.

/// Errors that can occur during kubeconfig parsing and loading.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum KubeconfigError {
    /// YAML content could not be parsed.
    #[error("failed to parse kubeconfig")]
    Parse(#[source] serde_yaml::Error),

    /// File could not be read from disk.
    #[error("failed to read kubeconfig")]
    Read(#[source] std::io::Error),

    /// A write target could not be uniquely resolved across
    /// multiple kubeconfig files.
    #[error("context write target is ambiguous across multiple kubeconfig files")]
    AmbiguousWrite,
}
