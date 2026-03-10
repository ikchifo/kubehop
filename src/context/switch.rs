// Rust guideline compliant 2026-02-21
//! Context switching via full `serde_yaml::Value` round-trip.

use std::path::Path;

use super::error::ContextError;
use super::yaml_helpers::{
    load_yaml_doc, read_current_context, set_current_context, validate_target_exists,
    write_yaml_doc,
};

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
/// through the write.
///
/// # Errors
///
/// - [`ContextError::NotFound`] if `target` does not match any entry
///   in the `contexts` array.
/// - [`ContextError::Kubeconfig`] for I/O or YAML parsing failures.
pub fn switch_context(
    path: impl AsRef<Path>,
    target: &str,
) -> Result<SwitchResult, ContextError> {
    let path = path.as_ref();

    let mut doc = load_yaml_doc(path)?;

    validate_target_exists(&doc, target)?;

    let previous = read_current_context(&doc);

    set_current_context(&mut doc, target);

    write_yaml_doc(path, &doc)?;

    Ok(SwitchResult {
        previous,
        current: target.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use serde_yaml::Value;

    use super::*;
    use crate::kubeconfig::KubeconfigError;

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
