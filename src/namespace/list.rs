//! Namespace listing via kubectl subprocess.

use std::path::{Path, PathBuf};
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
    list_namespaces_merged(&[kubeconfig_path.to_path_buf()])
}

/// List namespaces using kubectl's merged kubeconfig path semantics.
///
/// # Errors
///
/// - [`NamespaceError::KubectlNotFound`] if kubectl is not in PATH.
/// - [`NamespaceError::InvalidKubeconfigPaths`] if the paths cannot be joined.
/// - [`NamespaceError::ListFailed`] if the kubectl command fails.
pub fn list_namespaces_merged(kubeconfig_paths: &[PathBuf]) -> Result<Vec<String>, NamespaceError> {
    let mut command = Command::new("kubectl");
    command
        .arg("get")
        .arg("namespaces")
        .arg("-o")
        .arg("jsonpath={.items[*].metadata.name}");
    configure_kubeconfig(&mut command, kubeconfig_paths)?;

    let output = command.output().map_err(|e| {
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
    namespace_exists_merged(&[kubeconfig_path.to_path_buf()], namespace)
}

/// Check whether a namespace exists using merged kubeconfig path semantics.
///
/// # Errors
///
/// - [`NamespaceError::KubectlNotFound`] if kubectl is not in PATH.
/// - [`NamespaceError::InvalidKubeconfigPaths`] if the paths cannot be joined.
/// - [`NamespaceError::NotFound`] if the namespace does not exist.
/// - [`NamespaceError::ListFailed`] for other kubectl failures.
pub fn namespace_exists_merged(
    kubeconfig_paths: &[PathBuf],
    namespace: &str,
) -> Result<(), NamespaceError> {
    let mut command = Command::new("kubectl");
    command
        .arg("get")
        .arg("namespace")
        .arg(namespace)
        .arg("-o")
        .arg("name");
    configure_kubeconfig(&mut command, kubeconfig_paths)?;

    let output = command.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            NamespaceError::KubectlNotFound
        } else {
            NamespaceError::ListFailed(e.to_string())
        }
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr_lower = stderr.to_lowercase();
        if stderr_lower.contains("not found") || stderr_lower.contains("notfound") {
            return Err(NamespaceError::NotFound(namespace.to_owned()));
        }
        return Err(NamespaceError::ListFailed(stderr.trim().to_owned()));
    }

    Ok(())
}

fn configure_kubeconfig(
    command: &mut Command,
    kubeconfig_paths: &[PathBuf],
) -> Result<(), NamespaceError> {
    match kubeconfig_paths {
        [] => {}
        [path] => {
            command.arg("--kubeconfig").arg(path);
        }
        paths => {
            let value = std::env::join_paths(paths)
                .map_err(|error| NamespaceError::InvalidKubeconfigPaths(error.to_string()))?;
            command.env("KUBECONFIG", value);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_kubeconfig_uses_the_explicit_command_flag() {
        let path = PathBuf::from("/tmp/first.yaml");
        let mut command = Command::new("kubectl");

        configure_kubeconfig(&mut command, std::slice::from_ref(&path)).unwrap();

        let args = command.get_args().collect::<Vec<_>>();
        assert_eq!(args, ["--kubeconfig", path.to_str().unwrap()]);
        assert!(command.get_envs().all(|(key, _)| key != "KUBECONFIG"));
    }

    #[test]
    fn multiple_kubeconfigs_use_the_kubeconfig_environment_variable() {
        let paths = vec![
            PathBuf::from("/tmp/first.yaml"),
            PathBuf::from("/tmp/second.yaml"),
        ];
        let mut command = Command::new("kubectl");

        configure_kubeconfig(&mut command, &paths).unwrap();

        assert_eq!(command.get_args().count(), 0);
        let value = command
            .get_envs()
            .find_map(|(key, value)| (key == "KUBECONFIG").then_some(value).flatten())
            .expect("KUBECONFIG should be configured");
        assert_eq!(std::env::split_paths(value).collect::<Vec<_>>(), paths);
    }
}
