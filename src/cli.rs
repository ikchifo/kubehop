// Rust guideline compliant 2026-02-21
//! CLI argument parsing and application-level orchestration.

use std::path::PathBuf;

use crate::dispatch::ToolMode;

/// Top-level application configuration resolved from the environment.
#[derive(Debug, Clone)]
pub struct Config {
    /// The raw argv0 value used for tool mode dispatch.
    pub argv0: String,
    /// Kubeconfig file paths (from `KUBECONFIG` or default).
    pub kubeconfig_paths: Vec<PathBuf>,
    /// Cache directory for state files.
    pub cache_dir: PathBuf,
    /// Whether color output is force-enabled.
    pub force_color: bool,
    /// Whether color output is disabled (`NO_COLOR`).
    pub no_color: bool,
    /// Whether context switching is blocked (`KUBECTX_ISOLATED_SHELL`).
    pub isolated_shell: bool,
}

impl Config {
    /// Build configuration from environment variables and defaults.
    #[must_use]
    pub fn from_env() -> Self {
        let argv0 = std::env::args()
            .next()
            .unwrap_or_else(|| String::from("kubectx"));

        let kubeconfig_paths = resolve_kubeconfig_paths();
        let cache_dir = resolve_cache_dir();

        Self {
            argv0,
            kubeconfig_paths,
            cache_dir,
            force_color: std::env::var_os("_KUBECTX_FORCE_COLOR").is_some(),
            no_color: std::env::var_os("NO_COLOR").is_some(),
            isolated_shell: std::env::var_os("KUBECTX_ISOLATED_SHELL")
                .is_some_and(|v| v == "1"),
        }
    }
}

/// Execute the resolved command in the given tool mode.
///
/// # Errors
///
/// Returns an error if command execution fails.
pub fn execute(_mode: ToolMode, _config: Config) -> anyhow::Result<()> {
    // TODO: wire clap arg parsing and command dispatch
    Ok(())
}

fn resolve_kubeconfig_paths() -> Vec<PathBuf> {
    if let Ok(val) = std::env::var("KUBECONFIG") {
        val.split(':')
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .collect()
    } else {
        let home = directories::BaseDirs::new()
            .map(|d| d.home_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("~"));
        vec![home.join(".kube").join("config")]
    }
}

fn resolve_cache_dir() -> PathBuf {
    if let Ok(val) = std::env::var("XDG_CACHE_HOME") {
        PathBuf::from(val)
    } else {
        directories::BaseDirs::new()
            .map(|d| d.home_dir().join(".kube"))
            .unwrap_or_else(|| PathBuf::from("."))
    }
}
