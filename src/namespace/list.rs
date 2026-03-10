// Rust guideline compliant 2026-02-21
//! Namespace listing via kubectl subprocess.

use std::path::Path;
use std::process::Command;

use super::error::NamespaceError;

/// List namespaces from the cluster using kubectl.
///
/// Shells out to `kubectl get namespaces` with the given kubeconfig path.
/// Returns namespace names in API response order.
///
/// # Errors
///
/// - [`NamespaceError::KubectlNotFound`] if kubectl is not in PATH.
/// - [`NamespaceError::ListFailed`] if the kubectl command fails.
pub fn list_namespaces(kubeconfig_path: &Path) -> Result<Vec<String>, NamespaceError> {
    let output = Command::new("kubectl")
        .arg("get")
        .arg("namespaces")
        .arg("-o")
        .arg("jsonpath={.items[*].metadata.name}")
        .arg("--kubeconfig")
        .arg(kubeconfig_path)
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                NamespaceError::KubectlNotFound
            } else {
                NamespaceError::ListFailed(e.to_string())
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(NamespaceError::ListFailed(stderr.trim().to_owned()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let namespaces: Vec<String> = stdout
        .split_whitespace()
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();

    Ok(namespaces)
}

/// Check if a namespace exists on the cluster.
///
/// # Errors
///
/// - [`NamespaceError::KubectlNotFound`] if kubectl is not in PATH.
/// - [`NamespaceError::NotFound`] if the namespace does not exist.
/// - [`NamespaceError::ListFailed`] for other kubectl failures.
pub fn namespace_exists(kubeconfig_path: &Path, namespace: &str) -> Result<(), NamespaceError> {
    let output = Command::new("kubectl")
        .arg("get")
        .arg("namespace")
        .arg(namespace)
        .arg("--kubeconfig")
        .arg(kubeconfig_path)
        .arg("-o")
        .arg("name")
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                NamespaceError::KubectlNotFound
            } else {
                NamespaceError::ListFailed(e.to_string())
            }
        })?;

    if !output.status.success() {
        return Err(NamespaceError::NotFound(namespace.to_owned()));
    }

    Ok(())
}
