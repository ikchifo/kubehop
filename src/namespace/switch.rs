// Rust guideline compliant 2026-02-21
//! Namespace switching via full `serde_yaml::Value` round-trip.

use std::path::Path;

use serde_yaml::Value;

use super::error::NamespaceError;
use super::DEFAULT_NAMESPACE;
use crate::context::yaml_helpers::{load_yaml_doc, read_current_context, write_yaml_doc};

/// Outcome of a namespace switch operation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct NsSwitchResult {
    /// The context whose namespace was changed.
    pub context: String,
    /// The previous namespace (before the switch).
    pub previous: String,
    /// The new namespace (after the switch).
    pub current: String,
}

/// Outcome of a namespace unset operation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct NsUnsetResult {
    /// The context whose namespace was reset.
    pub context: String,
    /// The previous namespace before unsetting.
    pub previous: String,
}

/// Switch the namespace for the current context.
///
/// Performs a full `serde_yaml::Value` round-trip so that all fields
/// are preserved through the write.
///
/// # Errors
///
/// - [`NamespaceError::NoCurrentContext`] if no current context is set.
/// - [`NamespaceError::Kubeconfig`] for I/O or YAML failures.
pub fn switch_namespace(
    path: impl AsRef<Path>,
    namespace: &str,
) -> Result<NsSwitchResult, NamespaceError> {
    let path = path.as_ref();
    let mut doc = load_yaml_doc(path).map_err(NamespaceError::from_context_err)?;

    let ctx_name =
        read_current_context(&doc).ok_or(NamespaceError::NoCurrentContext)?;

    let previous = read_namespace_of_context(&doc, &ctx_name);
    set_namespace_in_context(&mut doc, &ctx_name, namespace);

    write_yaml_doc(path, &doc).map_err(NamespaceError::from_context_err)?;

    Ok(NsSwitchResult {
        context: ctx_name,
        previous,
        current: namespace.to_owned(),
    })
}

/// Reset the namespace for the current context to "default".
///
/// # Errors
///
/// - [`NamespaceError::NoCurrentContext`] if no current context is set.
/// - [`NamespaceError::Kubeconfig`] for I/O or YAML failures.
pub fn unset_namespace(path: impl AsRef<Path>) -> Result<NsUnsetResult, NamespaceError> {
    let path = path.as_ref();
    let mut doc = load_yaml_doc(path).map_err(NamespaceError::from_context_err)?;

    let ctx_name =
        read_current_context(&doc).ok_or(NamespaceError::NoCurrentContext)?;

    let previous = read_namespace_of_context(&doc, &ctx_name);
    set_namespace_in_context(&mut doc, &ctx_name, DEFAULT_NAMESPACE);

    write_yaml_doc(path, &doc).map_err(NamespaceError::from_context_err)?;

    Ok(NsUnsetResult {
        context: ctx_name,
        previous,
    })
}

fn read_namespace_of_context(doc: &Value, context_name: &str) -> String {
    let Some(contexts) = doc.get("contexts").and_then(Value::as_sequence) else {
        return DEFAULT_NAMESPACE.to_owned();
    };

    for entry in contexts {
        let matches = entry
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|n| n == context_name);

        if matches {
            return entry
                .get("context")
                .and_then(|c| c.get("namespace"))
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_NAMESPACE)
                .to_owned();
        }
    }

    DEFAULT_NAMESPACE.to_owned()
}

fn set_namespace_in_context(doc: &mut Value, context_name: &str, namespace: &str) {
    let Some(contexts) = doc.get_mut("contexts").and_then(Value::as_sequence_mut) else {
        return;
    };

    for entry in contexts {
        let matches = entry
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|n| n == context_name);

        if matches {
            if entry.get("context").is_none() {
                if let Value::Mapping(map) = entry {
                    map.insert(
                        Value::String("context".to_owned()),
                        Value::Mapping(serde_yaml::Mapping::new()),
                    );
                }
            }

            if let Some(Value::Mapping(ctx_map)) = entry.get_mut("context") {
                ctx_map.insert(
                    Value::String("namespace".to_owned()),
                    Value::String(namespace.to_owned()),
                );
            }
            break;
        }
    }
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

    const KUBECONFIG_WITH_NS: &str = "\
apiVersion: v1
kind: Config
current-context: dev
contexts:
  - name: dev
    context:
      cluster: dev-cluster
      namespace: kube-system
  - name: staging
    context:
      cluster: staging-cluster
clusters:
  - name: dev-cluster
    cluster:
      server: https://dev.example.com
";

    const KUBECONFIG_NO_NS: &str = "\
apiVersion: v1
kind: Config
current-context: dev
contexts:
  - name: dev
    context:
      cluster: dev-cluster
";

    const KUBECONFIG_NO_CURRENT: &str = "\
apiVersion: v1
kind: Config
contexts:
  - name: dev
    context:
      cluster: dev-cluster
";

    #[test]
    fn switch_changes_namespace_field() {
        let f = write_temp_kubeconfig(KUBECONFIG_WITH_NS);
        let result = switch_namespace(f.path(), "monitoring").unwrap();

        assert_eq!(result.current, "monitoring");

        let after = fs::read_to_string(f.path()).unwrap();
        let doc: Value = serde_yaml::from_str(&after).unwrap();
        let ns = doc["contexts"][0]["context"]["namespace"]
            .as_str()
            .unwrap();
        assert_eq!(ns, "monitoring");
    }

    #[test]
    fn switch_returns_previous_namespace() {
        let f = write_temp_kubeconfig(KUBECONFIG_WITH_NS);
        let result = switch_namespace(f.path(), "monitoring").unwrap();

        assert_eq!(result.previous, "kube-system");
        assert_eq!(result.context, "dev");
    }

    #[test]
    fn switch_defaults_previous_to_default_when_not_set() {
        let f = write_temp_kubeconfig(KUBECONFIG_NO_NS);
        let result = switch_namespace(f.path(), "monitoring").unwrap();

        assert_eq!(result.previous, "default");
    }

    #[test]
    fn switch_creates_namespace_field_when_absent() {
        let content = "\
apiVersion: v1
kind: Config
current-context: dev
contexts:
  - name: dev
";
        let f = write_temp_kubeconfig(content);
        let result = switch_namespace(f.path(), "production").unwrap();

        assert_eq!(result.current, "production");

        let after = fs::read_to_string(f.path()).unwrap();
        let doc: Value = serde_yaml::from_str(&after).unwrap();
        let ns = doc["contexts"][0]["context"]["namespace"]
            .as_str()
            .unwrap();
        assert_eq!(ns, "production");
    }

    #[test]
    fn unset_resets_to_default() {
        let f = write_temp_kubeconfig(KUBECONFIG_WITH_NS);
        let result = unset_namespace(f.path()).unwrap();

        assert_eq!(result.previous, "kube-system");
        assert_eq!(result.context, "dev");

        let after = fs::read_to_string(f.path()).unwrap();
        let doc: Value = serde_yaml::from_str(&after).unwrap();
        let ns = doc["contexts"][0]["context"]["namespace"]
            .as_str()
            .unwrap();
        assert_eq!(ns, "default");
    }

    #[test]
    fn switch_returns_error_when_no_current_context() {
        let f = write_temp_kubeconfig(KUBECONFIG_NO_CURRENT);
        let err = switch_namespace(f.path(), "monitoring").unwrap_err();
        assert!(matches!(err, NamespaceError::NoCurrentContext));
    }

    #[test]
    fn unset_returns_error_when_no_current_context() {
        let f = write_temp_kubeconfig(KUBECONFIG_NO_CURRENT);
        let err = unset_namespace(f.path()).unwrap_err();
        assert!(matches!(err, NamespaceError::NoCurrentContext));
    }

    #[test]
    fn switch_preserves_other_kubeconfig_fields() {
        let f = write_temp_kubeconfig(KUBECONFIG_WITH_NS);
        let _ = switch_namespace(f.path(), "monitoring").unwrap();

        let after = fs::read_to_string(f.path()).unwrap();
        let doc: Value = serde_yaml::from_str(&after).unwrap();

        let clusters = doc.get("clusters").and_then(Value::as_sequence);
        assert!(clusters.is_some(), "clusters field must survive round-trip");
        assert_eq!(clusters.unwrap().len(), 1);

        assert_eq!(
            doc.get("apiVersion").and_then(Value::as_str),
            Some("v1")
        );
    }
}
