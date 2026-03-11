//! Per-context previous-namespace state file persistence.

use std::fs;
use std::path::{Path, PathBuf};

use super::error::NamespaceError;

/// Manages per-context previous-namespace cache files.
///
/// Each context gets its own state file at `{cache_dir}/kubens/{context}`.
/// This matches the Go kubens state file layout for seamless migration.
#[derive(Debug)]
pub struct NsStateFile {
    path: PathBuf,
}

impl NsStateFile {
    /// Create a handle for the state file for a given context.
    ///
    /// Does not perform any I/O; the file is read or created lazily
    /// via [`load`](Self::load) and [`save`](Self::save).
    #[must_use]
    pub fn new(cache_dir: impl AsRef<Path>, context_name: &str) -> Self {
        let sanitized = sanitize_context_name(context_name);
        Self {
            path: cache_dir.as_ref().join("kubens").join(sanitized),
        }
    }

    /// Read the previously active namespace for this context.
    ///
    /// Returns `Ok(None)` when the state file does not exist yet
    /// (first run) or when it is empty.
    ///
    /// # Errors
    ///
    /// Returns [`NamespaceError::State`] if the file exists but cannot
    /// be read.
    pub fn load(&self) -> Result<Option<String>, NamespaceError> {
        match fs::read_to_string(&self.path) {
            Ok(content) => {
                let trimmed = content.trim();
                if trimmed.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(trimmed.to_owned()))
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(NamespaceError::State(e)),
        }
    }

    /// Persist the previous namespace to disk.
    ///
    /// Creates parent directories if they do not exist.
    ///
    /// # Errors
    ///
    /// Returns [`NamespaceError::State`] if the file or its parent
    /// directories cannot be created or written.
    pub fn save(&self, namespace: &str) -> Result<(), NamespaceError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(NamespaceError::State)?;
        }
        fs::write(&self.path, namespace).map_err(NamespaceError::State)
    }
}

fn sanitize_context_name(name: &str) -> String {
    name.replace(['/', '\\', ':'], "__").replace('\0', "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_returns_none_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let state = NsStateFile::new(dir.path(), "dev");
        assert_eq!(state.load().unwrap(), None);
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let state = NsStateFile::new(dir.path(), "dev");

        state.save("kube-system").unwrap();
        assert_eq!(state.load().unwrap().as_deref(), Some("kube-system"));
    }

    #[test]
    fn load_trims_whitespace() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kubens").join("dev");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "  monitoring\n  ").unwrap();

        let state = NsStateFile::new(dir.path(), "dev");
        assert_eq!(state.load().unwrap().as_deref(), Some("monitoring"));
    }

    #[test]
    fn load_returns_none_for_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kubens").join("dev");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "   \n  ").unwrap();

        let state = NsStateFile::new(dir.path(), "dev");
        assert_eq!(state.load().unwrap(), None);
    }

    #[test]
    fn save_creates_parent_directories() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("deep").join("nested");
        let state = NsStateFile::new(&nested, "prod");

        state.save("kube-public").unwrap();
        assert_eq!(state.load().unwrap().as_deref(), Some("kube-public"));
    }

    #[test]
    fn different_contexts_have_separate_state_files() {
        let dir = tempfile::tempdir().unwrap();
        let state_dev = NsStateFile::new(dir.path(), "dev");
        let state_prod = NsStateFile::new(dir.path(), "prod");

        state_dev.save("ns-alpha").unwrap();
        state_prod.save("ns-beta").unwrap();

        assert_eq!(state_dev.load().unwrap().as_deref(), Some("ns-alpha"));
        assert_eq!(state_prod.load().unwrap().as_deref(), Some("ns-beta"));
    }

    #[test]
    fn save_overwrites_previous_value() {
        let dir = tempfile::tempdir().unwrap();
        let state = NsStateFile::new(dir.path(), "dev");

        state.save("alpha").unwrap();
        state.save("beta").unwrap();
        assert_eq!(state.load().unwrap().as_deref(), Some("beta"));
    }
}
