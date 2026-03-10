// Rust guideline compliant 2026-02-21
//! Argv0-based dispatch for `kubectx` vs `kubens` behavior.

/// Determines whether the binary operates in context or namespace mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ToolMode {
    /// Context switching mode (`kubectx` / `kubectl-ctx`).
    Kubectx,
    /// Namespace switching mode (`kubens` / `kubectl-ns`).
    Kubens,
}

/// Resolve the tool mode from the binary name in argv0.
///
/// Extracts the filename component from the path and matches against
/// known binary names. Defaults to `Kubectx` for unrecognized names.
#[must_use]
pub fn mode_from_argv0(argv0: &str) -> ToolMode {
    let binary_name = std::path::Path::new(argv0)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(argv0);

    match binary_name {
        "kubens" | "kubectl-ns" => ToolMode::Kubens,
        _ => ToolMode::Kubectx,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kubectx_is_default() {
        assert_eq!(mode_from_argv0("khop"), ToolMode::Kubectx);
        assert_eq!(mode_from_argv0("kubectx"), ToolMode::Kubectx);
        assert_eq!(mode_from_argv0("kubectl-ctx"), ToolMode::Kubectx);
        assert_eq!(mode_from_argv0("anything"), ToolMode::Kubectx);
    }

    #[test]
    fn kubens_detected() {
        assert_eq!(mode_from_argv0("kubens"), ToolMode::Kubens);
        assert_eq!(mode_from_argv0("kubectl-ns"), ToolMode::Kubens);
    }

    #[test]
    fn full_paths_resolve_correctly() {
        assert_eq!(
            mode_from_argv0("/usr/local/bin/kubectx"),
            ToolMode::Kubectx
        );
        assert_eq!(
            mode_from_argv0("/usr/local/bin/kubens"),
            ToolMode::Kubens
        );
    }
}
