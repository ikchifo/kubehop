//! Previous-context state file persistence.

use std::fs;
use std::path::{Path, PathBuf};

use super::error::ContextError;

/// Manages the previous-context cache file.
///
/// Stores the name of the last active context so that the user can
/// toggle back with `kubectx -`. The file lives at
/// `{cache_dir}/kubectx` (typically `~/.kube/kubectx` or
/// `$XDG_CACHE_HOME/kubectx`).
#[derive(Debug)]
pub struct StateFile {
    path: PathBuf,
}

impl StateFile {
    /// Create a handle for the state file inside `cache_dir`.
    ///
    /// Does not perform any I/O; the file is read or created lazily
    /// via [`load`](Self::load) and [`save`](Self::save).
    #[must_use]
    pub fn new(cache_dir: impl AsRef<Path>) -> Self {
        Self {
            path: cache_dir.as_ref().join("kubectx"),
        }
    }

    /// Read the previously active context name.
    ///
    /// Returns `Ok(None)` when the state file does not exist yet
    /// (first run) or when it is empty.
    ///
    /// # Errors
    ///
    /// Returns [`ContextError::State`] if the file exists but cannot
    /// be read.
    pub fn load(&self) -> Result<Option<String>, ContextError> {
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
            Err(e) => Err(ContextError::State(e)),
        }
    }

    /// Persist the previous context name to disk.
    ///
    /// Creates parent directories if they do not exist.
    ///
    /// # Errors
    ///
    /// Returns [`ContextError::State`] if the file or its parent
    /// directories cannot be created or written.
    pub fn save(&self, previous: &str) -> Result<(), ContextError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(ContextError::State)?;
        }
        fs::write(&self.path, previous).map_err(ContextError::State)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_returns_none_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let state = StateFile::new(dir.path());
        assert_eq!(state.load().unwrap(), None);
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let state = StateFile::new(dir.path());

        state.save("production").unwrap();
        assert_eq!(state.load().unwrap().as_deref(), Some("production"));
    }

    #[test]
    fn load_trims_whitespace() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kubectx");
        fs::write(&path, "  staging\n  ").unwrap();

        let state = StateFile::new(dir.path());
        assert_eq!(state.load().unwrap().as_deref(), Some("staging"));
    }

    #[test]
    fn load_returns_none_for_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kubectx");
        fs::write(&path, "   \n  ").unwrap();

        let state = StateFile::new(dir.path());
        assert_eq!(state.load().unwrap(), None);
    }

    #[test]
    fn save_creates_parent_directories() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("deep").join("nested");
        let state = StateFile::new(&nested);

        state.save("dev").unwrap();
        assert_eq!(state.load().unwrap().as_deref(), Some("dev"));
    }

    #[test]
    fn save_overwrites_previous_value() {
        let dir = tempfile::tempdir().unwrap();
        let state = StateFile::new(dir.path());

        state.save("alpha").unwrap();
        state.save("beta").unwrap();
        assert_eq!(state.load().unwrap().as_deref(), Some("beta"));
    }
}
