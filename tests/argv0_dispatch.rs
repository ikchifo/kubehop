// Rust guideline compliant 2026-02-21
//! Integration tests for argv0-based tool mode dispatch.

use khop::dispatch::{mode_from_argv0, ToolMode};

#[test]
fn test_dispatch_kubectx_bare_name() {
    assert_eq!(mode_from_argv0("kubectx"), ToolMode::Kubectx);
}

#[test]
fn test_dispatch_kubens_bare_name() {
    assert_eq!(mode_from_argv0("kubens"), ToolMode::Kubens);
}

#[test]
fn test_dispatch_kubectl_ctx_plugin_name() {
    assert_eq!(mode_from_argv0("kubectl-ctx"), ToolMode::Kubectx);
}

#[test]
fn test_dispatch_kubectl_ns_plugin_name() {
    assert_eq!(mode_from_argv0("kubectl-ns"), ToolMode::Kubens);
}

#[test]
fn test_dispatch_full_path_kubens() {
    assert_eq!(mode_from_argv0("/usr/local/bin/kubens"), ToolMode::Kubens);
}

#[test]
fn test_dispatch_full_path_kubectx() {
    assert_eq!(mode_from_argv0("/usr/local/bin/kubectx"), ToolMode::Kubectx);
}

#[test]
fn test_dispatch_unknown_name_defaults_to_kubectx() {
    assert_eq!(mode_from_argv0("khop"), ToolMode::Kubectx);
    assert_eq!(mode_from_argv0("something-else"), ToolMode::Kubectx);
    assert_eq!(mode_from_argv0(""), ToolMode::Kubectx);
}
