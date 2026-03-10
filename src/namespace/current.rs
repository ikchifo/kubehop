// Rust guideline compliant 2026-02-21
//! Current namespace resolution.

use crate::kubeconfig::KubeConfigView;

use super::error::NamespaceError;
use super::DEFAULT_NAMESPACE;

/// Resolve the active namespace for the current context.
///
/// Returns "default" if no namespace field is set on the context,
/// matching kubectl behavior.
///
/// # Errors
///
/// Returns [`NamespaceError::NoCurrentContext`] if no current context is set.
pub fn current_namespace(view: &KubeConfigView) -> Result<String, NamespaceError> {
    let ctx_name = view
        .current_context()
        .ok_or(NamespaceError::NoCurrentContext)?;

    let ns = view
        .contexts
        .iter()
        .find(|c| c.name == ctx_name)
        .and_then(|c| c.context.as_ref())
        .and_then(|f| f.namespace.as_deref())
        .unwrap_or(DEFAULT_NAMESPACE);

    Ok(ns.to_owned())
}

#[cfg(test)]
mod tests {
    use crate::kubeconfig::{ContextEntry, ContextFields};

    use super::*;

    #[test]
    fn returns_default_when_no_namespace_field() {
        let view = KubeConfigView {
            current_context: Some("dev".to_owned()),
            contexts: vec![ContextEntry {
                name: "dev".to_owned(),
                context: Some(ContextFields { namespace: None }),
            }],
        };
        assert_eq!(current_namespace(&view).unwrap(), "default");
    }

    #[test]
    fn returns_namespace_when_set() {
        let view = KubeConfigView {
            current_context: Some("dev".to_owned()),
            contexts: vec![ContextEntry {
                name: "dev".to_owned(),
                context: Some(ContextFields {
                    namespace: Some("kube-system".to_owned()),
                }),
            }],
        };
        assert_eq!(current_namespace(&view).unwrap(), "kube-system");
    }

    #[test]
    fn returns_default_when_context_field_is_none() {
        let view = KubeConfigView {
            current_context: Some("dev".to_owned()),
            contexts: vec![ContextEntry {
                name: "dev".to_owned(),
                context: None,
            }],
        };
        assert_eq!(current_namespace(&view).unwrap(), "default");
    }

    #[test]
    fn returns_default_when_context_not_in_list() {
        let view = KubeConfigView {
            current_context: Some("missing".to_owned()),
            contexts: vec![ContextEntry {
                name: "dev".to_owned(),
                context: Some(ContextFields { namespace: None }),
            }],
        };
        assert_eq!(current_namespace(&view).unwrap(), "default");
    }

    #[test]
    fn returns_error_when_no_current_context() {
        let view = KubeConfigView {
            current_context: None,
            contexts: vec![],
        };
        let err = current_namespace(&view).unwrap_err();
        assert!(matches!(err, NamespaceError::NoCurrentContext));
    }
}
