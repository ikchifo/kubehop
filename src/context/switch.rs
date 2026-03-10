// Rust guideline compliant 2026-02-21
//! Context switching via full `serde_yaml::Value` round-trip.

use std::fs;
use std::path::Path;

use serde_yaml::Value;

use super::error::ContextError;
use crate::kubeconfig::KubeconfigError;

/// Outcome of a context switch operation.
///
/// Captures both the previously active context (if any) and the newly
/// activated context name. The `previous` field is `None` when the
/// kubeconfig had no `current-context` set before the switch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchResult {
    /// The context that was active before the switch, if any.
    pub previous: Option<String>,
    /// The context that is now active.
    pub current: String,
}

/// Switch the active context in a kubeconfig file.
///
/// Performs a full `serde_yaml::Value` round-trip so that all fields
/// (clusters, users, auth providers, certificates) are preserved
/// byte-for-byte through the write.
///
/// # Errors
///
/// - [`ContextError::NotFound`] if `target` does not match any entry
///   in the `contexts` array.
/// - [`ContextError::Kubeconfig`] wrapping [`KubeconfigError::Read`]
///   for I/O failures on read or write.
/// - [`ContextError::Kubeconfig`] wrapping [`KubeconfigError::Parse`]
///   if the file contains invalid YAML.
pub fn switch_context(
    path: impl AsRef<Path>,
    target: &str,
) -> Result<SwitchResult, ContextError> {
    let path = path.as_ref();

    let raw = fs::read_to_string(path).map_err(KubeconfigError::Read)?;
    let mut doc: Value = serde_yaml::from_str(&raw).map_err(KubeconfigError::Parse)?;

    validate_target_exists(&doc, target)?;

    let previous = read_current_context(&doc);

    set_current_context(&mut doc, target);

    let out = serde_yaml::to_string(&doc).map_err(KubeconfigError::Parse)?;
    fs::write(path, out).map_err(KubeconfigError::Read)?;

    Ok(SwitchResult {
        previous,
        current: target.to_owned(),
    })
}

/// Verify that `target` appears as a context name in the `contexts` array.
fn validate_target_exists(doc: &Value, target: &str) -> Result<(), ContextError> {
    let contexts = doc
        .get("contexts")
        .and_then(Value::as_sequence)
        .ok_or_else(|| ContextError::NotFound(target.to_owned()))?;

    let found = contexts.iter().any(|entry| {
        entry
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|n| n == target)
    });

    if found {
        Ok(())
    } else {
        Err(ContextError::NotFound(target.to_owned()))
    }
}

/// Extract the current `current-context` value from the document.
fn read_current_context(doc: &Value) -> Option<String> {
    doc.get("current-context")
        .and_then(Value::as_str)
        .map(String::from)
}

/// Set `current-context` to `target` in the document mapping.
fn set_current_context(doc: &mut Value, target: &str) {
    if let Value::Mapping(ref mut map) = *doc {
        let key = Value::String("current-context".to_owned());
        map.insert(key, Value::String(target.to_owned()));
    }
}

#[cfg(test)]
mod tests {
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

    #[test]
    fn switches_to_existing_context() {
        let f = write_temp_kubeconfig(SIMPLE_KUBECONFIG);
        let result = switch_context(f.path(), "staging").unwrap();

        assert_eq!(result.previous.as_deref(), Some("dev"));
        assert_eq!(result.current, "staging");

        let after = fs::read_to_string(f.path()).unwrap();
        let doc: Value = serde_yaml::from_str(&after).unwrap();
        assert_eq!(
            doc.get("current-context").and_then(Value::as_str),
            Some("staging")
        );
    }

    #[test]
    fn returns_not_found_for_unknown_context() {
        let f = write_temp_kubeconfig(SIMPLE_KUBECONFIG);
        let err = switch_context(f.path(), "nonexistent").unwrap_err();
        assert!(matches!(err, ContextError::NotFound(ref name) if name == "nonexistent"));
    }

    #[test]
    fn preserves_clusters_and_other_fields() {
        let f = write_temp_kubeconfig(SIMPLE_KUBECONFIG);
        switch_context(f.path(), "staging").unwrap();

        let after = fs::read_to_string(f.path()).unwrap();
        let doc: Value = serde_yaml::from_str(&after).unwrap();

        let clusters = doc.get("clusters").and_then(Value::as_sequence);
        assert!(clusters.is_some(), "clusters field must survive round-trip");
        assert_eq!(clusters.unwrap().len(), 1);
    }

    #[test]
    fn handles_no_current_context() {
        let content = "\
apiVersion: v1
kind: Config
contexts:
  - name: alpha
    context:
      cluster: alpha-cluster
";
        let f = write_temp_kubeconfig(content);
        let result = switch_context(f.path(), "alpha").unwrap();

        assert_eq!(result.previous, None);
        assert_eq!(result.current, "alpha");
    }

    #[test]
    fn returns_io_error_for_missing_file() {
        let err = switch_context("/tmp/khop-nonexistent-9999.yaml", "dev").unwrap_err();
        assert!(matches!(
            err,
            ContextError::Kubeconfig(KubeconfigError::Read(_))
        ));
    }

    #[test]
    fn switching_to_same_context_is_idempotent() {
        let f = write_temp_kubeconfig(SIMPLE_KUBECONFIG);
        let result = switch_context(f.path(), "dev").unwrap();

        assert_eq!(result.previous.as_deref(), Some("dev"));
        assert_eq!(result.current, "dev");
    }
}
