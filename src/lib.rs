// Rust guideline compliant 2026-02-21
//! Fast Kubernetes context and namespace switcher.
//!
//! `khop` is the library backing the `kubectx` and `kubens` binaries.
//! It provides kubeconfig parsing, context switching, and an interactive
//! fuzzy picker.

pub mod cli;
pub mod completion;
pub mod context;
pub mod dispatch;
pub mod integration;
pub mod kubeconfig;
pub mod namespace;
pub mod picker;

pub use cli::Config;

/// Run the application with the given configuration.
///
/// # Errors
///
/// Returns an error if command parsing or execution fails.
pub fn run(config: &Config) -> anyhow::Result<()> {
    let mode = dispatch::mode_from_argv0(&config.argv0);
    cli::execute(mode, config)
}
