//! Shared helpers for `serde_yaml::Value` operations on kubeconfig documents.

use std::fs;
use std::io::Write as _;
use std::path::Path;

use serde_yaml::Value;

use super::error::ContextError;
use crate::kubeconfig::KubeconfigError;

const KEY_CURRENT_CONTEXT: &str = "current-context";
const KEY_CONTEXTS: &str = "contexts";
const KEY_NAME: &str = "name";

/// Load a kubeconfig file into a generic YAML document.
pub(crate) fn load_yaml_doc(path: &Path) -> Result<Value, ContextError> {
    let raw = fs::read_to_string(path).map_err(|source| KubeconfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    serde_yaml::from_str(&raw)
        .map_err(KubeconfigError::Parse)
        .map_err(Into::into)
}

/// Serialize and write a YAML document back to disk.
pub(crate) fn write_yaml_doc(path: &Path, doc: &Value) -> Result<(), ContextError> {
    let out = serde_yaml::to_string(doc).map_err(KubeconfigError::Parse)?;
    let target = fs::canonicalize(path).map_err(|source| write_error(path, source))?;
    let metadata = fs::metadata(&target).map_err(|source| write_error(path, source))?;
    let parent = target
        .parent()
        .ok_or_else(|| write_error(path, std::io::Error::other("kubeconfig has no parent")))?;

    let mut temp =
        tempfile::NamedTempFile::new_in(parent).map_err(|source| write_error(path, source))?;
    temp.write_all(out.as_bytes())
        .map_err(|source| write_error(path, source))?;
    temp.flush().map_err(|source| write_error(path, source))?;
    temp.as_file()
        .set_permissions(metadata.permissions())
        .map_err(|source| write_error(path, source))?;
    temp.as_file()
        .sync_all()
        .map_err(|source| write_error(path, source))?;
    temp.persist(&target)
        .map_err(|error| write_error(path, error.error))?;

    sync_parent_directory(parent).map_err(|source| write_error(path, source))?;
    Ok(())
}

fn write_error(path: &Path, source: std::io::Error) -> ContextError {
    KubeconfigError::Write {
        path: path.to_path_buf(),
        source,
    }
    .into()
}

#[cfg(unix)]
fn sync_parent_directory(parent: &Path) -> std::io::Result<()> {
    fs::File::open(parent)?.sync_all()
}

#[cfg(not(unix))]
fn sync_parent_directory(_parent: &Path) -> std::io::Result<()> {
    Ok(())
}

/// Verify that `target` appears as a context name in the `contexts` array.
pub(super) fn validate_target_exists(doc: &Value, target: &str) -> Result<(), ContextError> {
    if context_exists(doc, target) {
        Ok(())
    } else {
        Err(ContextError::NotFound(target.to_owned()))
    }
}

pub(super) fn context_exists(doc: &Value, target: &str) -> bool {
    doc.get(KEY_CONTEXTS)
        .and_then(Value::as_sequence)
        .is_some_and(|contexts| {
            contexts.iter().any(|entry| {
                entry
                    .get(KEY_NAME)
                    .and_then(Value::as_str)
                    .is_some_and(|name| name == target)
            })
        })
}

/// Set `current-context` to `target` in the document mapping.
pub(super) fn set_current_context(doc: &mut Value, target: &str) {
    if let Value::Mapping(map) = doc {
        let key = Value::String(KEY_CURRENT_CONTEXT.to_owned());
        map.insert(key, Value::String(target.to_owned()));
    }
}

/// Remove the `current-context` key from the document mapping.
pub(super) fn remove_current_context(doc: &mut Value) {
    if let Value::Mapping(map) = doc {
        let key = Value::String(KEY_CURRENT_CONTEXT.to_owned());
        map.remove(&key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KUBECONFIG: &str = "\
apiVersion: v1
kind: Config
current-context: dev
contexts:
  - name: dev
    context:
      cluster: dev-cluster
";

    #[cfg(unix)]
    #[test]
    fn write_yaml_doc_atomically_replaces_the_file() {
        use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        fs::write(&path, KUBECONFIG).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        let original_inode = fs::metadata(&path).unwrap().ino();
        let doc = load_yaml_doc(&path).unwrap();

        write_yaml_doc(&path, &doc).unwrap();

        let metadata = fs::metadata(&path).unwrap();
        assert_ne!(metadata.ino(), original_inode);
        assert_eq!(metadata.permissions().mode() & 0o777, 0o640);
    }

    #[cfg(unix)]
    #[test]
    fn write_yaml_doc_preserves_a_symlink_and_updates_its_target() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target-config");
        let link = dir.path().join("config");
        fs::write(&target, KUBECONFIG).unwrap();
        symlink(&target, &link).unwrap();
        let mut doc = load_yaml_doc(&link).unwrap();
        set_current_context(&mut doc, "staging");

        write_yaml_doc(&link, &doc).unwrap();

        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let updated = fs::read_to_string(&target).unwrap();
        assert!(updated.contains("current-context: staging"));
    }
}
