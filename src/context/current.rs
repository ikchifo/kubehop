// Rust guideline compliant 2026-02-21
//! Current context lookup.

use crate::kubeconfig::KubeConfigView;

/// Return the current context name from a kubeconfig view.
///
/// # Errors
///
/// Returns [`ContextError::NoContexts`](super::ContextError::NoContexts) if
/// `current-context` is not set in the kubeconfig.
pub fn current_context(view: &KubeConfigView) -> Result<&str, super::ContextError> {
    view.current_context()
        .ok_or(super::ContextError::NoContexts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_view(current: Option<&str>) -> KubeConfigView {
        KubeConfigView {
            current_context: current.map(String::from),
            contexts: Vec::new(),
        }
    }

    #[test]
    fn returns_current_when_set() {
        let view = make_view(Some("my-cluster"));
        let result = current_context(&view).unwrap();
        assert_eq!(result, "my-cluster");
    }

    #[test]
    fn returns_error_when_none() {
        let view = make_view(None);
        let err = current_context(&view).unwrap_err();
        assert!(
            matches!(err, super::super::ContextError::NoContexts),
            "expected NoContexts, got {err:?}"
        );
    }
}
