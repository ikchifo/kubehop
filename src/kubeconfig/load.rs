//! Kubeconfig file loading with sans-I/O core.

use std::path::{Path, PathBuf};

use super::error::KubeconfigError;
use super::model::KubeConfigView;

impl KubeConfigView {
    /// Parse a kubeconfig from any reader.
    ///
    /// This is the sans-I/O core. Unit tests can pass `&[u8]` or
    /// `std::io::Cursor` directly without touching the filesystem.
    ///
    /// # Errors
    ///
    /// Returns `KubeconfigError::Parse` if the YAML is malformed or
    /// does not match the expected kubeconfig structure.
    pub fn from_reader(reader: impl std::io::Read) -> Result<Self, KubeconfigError> {
        serde_yaml::from_reader(reader).map_err(KubeconfigError::Parse)
    }

    /// Load a kubeconfig from a single file path.
    ///
    /// # Errors
    ///
    /// Returns `KubeconfigError::Read` if the file cannot be opened,
    /// or `KubeconfigError::Parse` if the YAML is invalid.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, KubeconfigError> {
        let path = path.as_ref();
        let file = std::fs::File::open(path).map_err(|source| KubeconfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let reader = std::io::BufReader::new(file);
        Self::from_reader(reader)
    }

    /// Load and merge kubeconfigs from multiple file paths.
    ///
    /// Context names are deduplicated with first-occurrence-wins
    /// semantics, matching `kubectl` behavior. The `current-context`
    /// is taken from the first file that defines one.
    ///
    /// # Errors
    ///
    /// Returns an error if any individual file cannot be loaded.
    pub fn load_merged(paths: &[PathBuf]) -> Result<Self, KubeconfigError> {
        if paths.is_empty() {
            return Ok(Self {
                current_context: None,
                contexts: Vec::new(),
            });
        }

        if paths.len() == 1 {
            return Self::load(&paths[0]);
        }

        let mut current_context: Option<String> = None;
        let mut seen_names = std::collections::HashSet::new();
        let mut merged_contexts = Vec::new();

        for path in paths {
            let view = Self::load(path)?;

            if current_context.is_none() {
                current_context = view.current_context;
            }

            for entry in view.contexts {
                if seen_names.insert(entry.name.clone()) {
                    merged_contexts.push(entry);
                }
            }
        }

        Ok(Self {
            current_context,
            contexts: merged_contexts,
        })
    }
}
