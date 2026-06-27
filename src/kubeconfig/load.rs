//! Kubeconfig file loading with sans-I/O core.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::error::KubeconfigError;
use super::model::KubeConfigView;

#[derive(Debug, Clone)]
pub(crate) struct MergedKubeConfig {
    view: KubeConfigView,
    context_sources: HashMap<String, PathBuf>,
    current_context_source: Option<PathBuf>,
    primary_path: Option<PathBuf>,
}

impl MergedKubeConfig {
    pub(crate) fn load(paths: &[PathBuf]) -> Result<Self, KubeconfigError> {
        let mut current_context = None;
        let mut current_context_source = None;
        let mut context_sources = HashMap::new();
        let mut contexts = Vec::new();

        for path in paths {
            let view = KubeConfigView::load(path)?;

            if current_context.is_none()
                && let Some(name) = view.current_context
            {
                current_context = Some(name);
                current_context_source = Some(path.clone());
            }

            for entry in view.contexts {
                if !context_sources.contains_key(&entry.name) {
                    context_sources.insert(entry.name.clone(), path.clone());
                    contexts.push(entry);
                }
            }
        }

        Ok(Self {
            view: KubeConfigView {
                current_context,
                contexts,
            },
            context_sources,
            current_context_source,
            primary_path: paths.first().cloned(),
        })
    }

    pub(crate) const fn view(&self) -> &KubeConfigView {
        &self.view
    }

    pub(crate) fn into_view(self) -> KubeConfigView {
        self.view
    }

    pub(crate) fn context_source(&self, name: &str) -> Option<&Path> {
        self.context_sources.get(name).map(PathBuf::as_path)
    }

    pub(crate) fn current_context_source(&self) -> Option<&Path> {
        self.current_context_source.as_deref()
    }

    pub(crate) fn primary_path(&self) -> Option<&Path> {
        self.primary_path.as_deref()
    }
}

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
        MergedKubeConfig::load(paths).map(MergedKubeConfig::into_view)
    }
}
