// Rust guideline compliant 2026-02-21
//! Context mutation: rename, delete, unset.

use std::path::Path;

use serde_yaml::Value;

use super::error::ContextError;
use super::yaml_helpers::{
    load_yaml_doc, read_current_context, remove_current_context, set_current_context,
    validate_target_exists, write_yaml_doc,
};

/// Outcome of a context rename operation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct RenameResult {
    /// The original context name before renaming.
    pub old_name: String,
    /// The new context name after renaming.
    pub new_name: String,
}

/// Outcome of a context delete operation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct DeleteResult {
    /// The name of the deleted context.
    pub deleted: String,
    /// Whether the deleted context was the active `current-context`.
    pub was_current: bool,
}

/// Outcome of unsetting the active context.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct UnsetResult {
    /// The previously active context, if any was set.
    pub previous: Option<String>,
}

/// Rename a context in a kubeconfig file.
///
/// Performs a full `serde_yaml::Value` round-trip so that all fields
/// are preserved through the write. If the renamed context was the
/// active `current-context`, that field is updated to reflect the new
/// name.
///
/// # Errors
///
/// - [`ContextError::NotFound`] if `old` does not match any entry in
///   the `contexts` array.
/// - [`ContextError::Kubeconfig`] for I/O or YAML parsing failures.
pub fn rename_context(
    path: impl AsRef<Path>,
    old: &str,
    new_name: &str,
) -> Result<RenameResult, ContextError> {
    let path = path.as_ref();
    let mut doc = load_yaml_doc(path)?;

    validate_target_exists(&doc, old)?;
    rename_in_contexts(&mut doc, old, new_name);

    if read_current_context(&doc).as_deref() == Some(old) {
        set_current_context(&mut doc, new_name);
    }

    write_yaml_doc(path, &doc)?;

    Ok(RenameResult {
        old_name: old.to_owned(),
        new_name: new_name.to_owned(),
    })
}

/// Delete a context from a kubeconfig file.
///
/// Performs a full `serde_yaml::Value` round-trip so that all fields
/// are preserved through the write. If the deleted context was the
/// active `current-context`, that field is removed from the document.
///
/// # Errors
///
/// - [`ContextError::NotFound`] if `target` does not match any entry
///   in the `contexts` array.
/// - [`ContextError::Kubeconfig`] for I/O or YAML parsing failures.
pub fn delete_context(path: impl AsRef<Path>, target: &str) -> Result<DeleteResult, ContextError> {
    let path = path.as_ref();
    let mut doc = load_yaml_doc(path)?;

    validate_target_exists(&doc, target)?;

    let was_current = read_current_context(&doc).as_deref() == Some(target);

    remove_from_contexts(&mut doc, target);

    if was_current {
        remove_current_context(&mut doc);
    }

    write_yaml_doc(path, &doc)?;

    Ok(DeleteResult {
        deleted: target.to_owned(),
        was_current,
    })
}

/// Delete the currently active context from a kubeconfig file.
///
/// Resolves `current-context` from the document and removes both the
/// context entry and the `current-context` field in a single read-write
/// pass, avoiding a redundant parse.
///
/// # Errors
///
/// - [`ContextError::NoContexts`] if no `current-context` is set.
/// - [`ContextError::Kubeconfig`] for I/O or YAML parsing failures.
pub fn delete_current_context(path: impl AsRef<Path>) -> Result<DeleteResult, ContextError> {
    let path = path.as_ref();
    let mut doc = load_yaml_doc(path)?;

    let target = read_current_context(&doc).ok_or(ContextError::NoContexts)?;

    remove_from_contexts(&mut doc, &target);
    remove_current_context(&mut doc);

    write_yaml_doc(path, &doc)?;

    Ok(DeleteResult {
        deleted: target,
        was_current: true,
    })
}

/// Unset the active context in a kubeconfig file.
///
/// Removes the `current-context` key from the document entirely. All
/// other fields (contexts, clusters, users) are preserved. Returns
/// early without writing if no `current-context` was set.
///
/// # Errors
///
/// - [`ContextError::Kubeconfig`] for I/O or YAML parsing failures.
pub fn unset_context(path: impl AsRef<Path>) -> Result<UnsetResult, ContextError> {
    let path = path.as_ref();
    let mut doc = load_yaml_doc(path)?;

    let previous = read_current_context(&doc);
    if previous.is_none() {
        return Ok(UnsetResult { previous: None });
    }

    remove_current_context(&mut doc);
    write_yaml_doc(path, &doc)?;

    Ok(UnsetResult { previous })
}

fn rename_in_contexts(doc: &mut Value, old: &str, new_name: &str) {
    let Some(contexts) = doc.get_mut("contexts").and_then(Value::as_sequence_mut) else {
        return;
    };

    for entry in contexts {
        let matches = entry
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|n| n == old);

        if matches {
            if let Value::Mapping(map) = entry {
                let key = Value::String("name".to_owned());
                map.insert(key, Value::String(new_name.to_owned()));
            }
            break;
        }
    }
}

fn remove_from_contexts(doc: &mut Value, target: &str) {
    let Some(contexts) = doc.get_mut("contexts").and_then(Value::as_sequence_mut) else {
        return;
    };

    contexts.retain(|entry| {
        entry
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|n| n != target)
    });
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use super::*;

    fn write_temp_kubeconfig(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().expect("create temp file");
        f.write_all(content.as_bytes())
            .expect("write temp kubeconfig");
        f.flush().expect("flush temp file");
        f
    }

    const SIMPLE_KUBECONFIG: &str = "\
apiVersion: v1
kind: Config
current-context: dev
contexts:
  - name: dev
    context:
      cluster: dev-cluster
  - name: staging
    context:
      cluster: staging-cluster
clusters:
  - name: dev-cluster
    cluster:
      server: https://dev.example.com
";

    // -- rename --

    #[test]
    fn rename_existing_context() {
        let f = write_temp_kubeconfig(SIMPLE_KUBECONFIG);
        let result = rename_context(f.path(), "staging", "production").unwrap();

        assert_eq!(result.old_name, "staging");
        assert_eq!(result.new_name, "production");

        let after = fs::read_to_string(f.path()).unwrap();
        let doc: Value = serde_yaml::from_str(&after).unwrap();
        let names: Vec<&str> = doc
            .get("contexts")
            .and_then(Value::as_sequence)
            .unwrap()
            .iter()
            .filter_map(|e| e.get("name").and_then(Value::as_str))
            .collect();

        assert!(names.contains(&"production"));
        assert!(!names.contains(&"staging"));
    }

    #[test]
    fn rename_updates_current_context() {
        let f = write_temp_kubeconfig(SIMPLE_KUBECONFIG);
        let result = rename_context(f.path(), "dev", "development").unwrap();

        assert_eq!(result.old_name, "dev");
        assert_eq!(result.new_name, "development");

        let after = fs::read_to_string(f.path()).unwrap();
        let doc: Value = serde_yaml::from_str(&after).unwrap();
        assert_eq!(
            doc.get("current-context").and_then(Value::as_str),
            Some("development")
        );
    }

    #[test]
    fn rename_nonexistent_returns_not_found() {
        let f = write_temp_kubeconfig(SIMPLE_KUBECONFIG);
        let err = rename_context(f.path(), "nonexistent", "new").unwrap_err();
        assert!(matches!(err, ContextError::NotFound(ref name) if name == "nonexistent"));
    }

    #[test]
    fn rename_preserves_other_fields() {
        let f = write_temp_kubeconfig(SIMPLE_KUBECONFIG);
        let _ = rename_context(f.path(), "staging", "production").unwrap();

        let after = fs::read_to_string(f.path()).unwrap();
        let doc: Value = serde_yaml::from_str(&after).unwrap();

        let clusters = doc.get("clusters").and_then(Value::as_sequence);
        assert!(clusters.is_some(), "clusters field must survive round-trip");
        assert_eq!(clusters.unwrap().len(), 1);
    }

    // -- delete --

    #[test]
    fn delete_existing_context() {
        let f = write_temp_kubeconfig(SIMPLE_KUBECONFIG);
        let result = delete_context(f.path(), "staging").unwrap();

        assert_eq!(result.deleted, "staging");
        assert!(!result.was_current);

        let after = fs::read_to_string(f.path()).unwrap();
        let doc: Value = serde_yaml::from_str(&after).unwrap();
        let names: Vec<&str> = doc
            .get("contexts")
            .and_then(Value::as_sequence)
            .unwrap()
            .iter()
            .filter_map(|e| e.get("name").and_then(Value::as_str))
            .collect();

        assert!(!names.contains(&"staging"));
    }

    #[test]
    fn delete_current_context_unsets_current() {
        let f = write_temp_kubeconfig(SIMPLE_KUBECONFIG);
        let result = delete_context(f.path(), "dev").unwrap();

        assert_eq!(result.deleted, "dev");
        assert!(result.was_current);

        let after = fs::read_to_string(f.path()).unwrap();
        let doc: Value = serde_yaml::from_str(&after).unwrap();
        assert!(doc.get("current-context").is_none());
    }

    #[test]
    fn delete_nonexistent_returns_not_found() {
        let f = write_temp_kubeconfig(SIMPLE_KUBECONFIG);
        let err = delete_context(f.path(), "nonexistent").unwrap_err();
        assert!(matches!(err, ContextError::NotFound(ref name) if name == "nonexistent"));
    }

    #[test]
    fn delete_preserves_remaining_contexts() {
        let f = write_temp_kubeconfig(SIMPLE_KUBECONFIG);
        let _ = delete_context(f.path(), "staging").unwrap();

        let after = fs::read_to_string(f.path()).unwrap();
        let doc: Value = serde_yaml::from_str(&after).unwrap();
        let contexts = doc.get("contexts").and_then(Value::as_sequence).unwrap();

        assert_eq!(contexts.len(), 1);
        assert_eq!(contexts[0].get("name").and_then(Value::as_str), Some("dev"));

        let clusters = doc.get("clusters").and_then(Value::as_sequence);
        assert!(clusters.is_some(), "clusters field must survive round-trip");
    }

    // -- delete_current_context --

    #[test]
    fn delete_current_context_single_pass() {
        let f = write_temp_kubeconfig(SIMPLE_KUBECONFIG);
        let result = delete_current_context(f.path()).unwrap();

        assert_eq!(result.deleted, "dev");
        assert!(result.was_current);

        let after = fs::read_to_string(f.path()).unwrap();
        let doc: Value = serde_yaml::from_str(&after).unwrap();
        assert!(doc.get("current-context").is_none());
    }

    #[test]
    fn delete_current_context_errors_when_none_set() {
        let content = "\
apiVersion: v1
kind: Config
contexts:
  - name: alpha
    context:
      cluster: alpha-cluster
";
        let f = write_temp_kubeconfig(content);
        let err = delete_current_context(f.path()).unwrap_err();
        assert!(matches!(err, ContextError::NoContexts));
    }

    // -- unset --

    #[test]
    fn unset_removes_current_context() {
        let f = write_temp_kubeconfig(SIMPLE_KUBECONFIG);
        let result = unset_context(f.path()).unwrap();

        assert_eq!(result.previous.as_deref(), Some("dev"));

        let after = fs::read_to_string(f.path()).unwrap();
        let doc: Value = serde_yaml::from_str(&after).unwrap();
        assert!(doc.get("current-context").is_none());
    }

    #[test]
    fn unset_when_no_current_context() {
        let content = "\
apiVersion: v1
kind: Config
contexts:
  - name: alpha
    context:
      cluster: alpha-cluster
";
        let f = write_temp_kubeconfig(content);
        let result = unset_context(f.path()).unwrap();

        assert_eq!(result.previous, None);
    }

    #[test]
    fn unset_preserves_contexts_list() {
        let f = write_temp_kubeconfig(SIMPLE_KUBECONFIG);
        let _ = unset_context(f.path()).unwrap();

        let after = fs::read_to_string(f.path()).unwrap();
        let doc: Value = serde_yaml::from_str(&after).unwrap();
        let contexts = doc.get("contexts").and_then(Value::as_sequence).unwrap();

        assert_eq!(contexts.len(), 2);
    }
}
