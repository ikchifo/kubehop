// Rust guideline compliant 2026-02-21
//! k9s plugin subcommand handler.

use std::path::PathBuf;

use anyhow::{bail, Context as _};

use crate::cli::Config;
use crate::context::{list, switch};
use crate::kubeconfig::KubeConfigView;
use crate::picker::{self, PickerItem, PickerResult};

/// Arguments for the `pick` subcommand used by k9s plugin integration.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PickArgs {
    pub switch: bool,
    pub kubeconfig: Option<PathBuf>,
    pub current: Option<String>,
}

/// Parse `pick` subcommand arguments from the args slice (after "pick"
/// has been consumed).
///
/// # Errors
///
/// Returns an error if an unknown flag is encountered or if a flag that
/// requires a value is missing its argument.
pub fn parse_pick_args(args: &[String]) -> anyhow::Result<PickArgs> {
    let mut result = PickArgs::default();
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "--switch" => {
                result.switch = true;
            }
            "--kubeconfig" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| anyhow::anyhow!("--kubeconfig requires a path argument"))?;
                result.kubeconfig = Some(PathBuf::from(value));
            }
            "--current" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| anyhow::anyhow!("--current requires a context name"))?;
                result.current = Some(value.clone());
            }
            other => bail!("unknown pick flag: {other}"),
        }
        i += 1;
    }

    Ok(result)
}

/// Execute the k9s pick subcommand.
///
/// Loads the kubeconfig, presents the interactive picker, and optionally
/// switches to the selected context when `--switch` is set. Without
/// `--switch`, the selected context name is printed to stdout.
///
/// # Errors
///
/// Returns an error if kubeconfig loading, the picker, or context
/// switching fails.
pub fn execute_pick(args: &PickArgs, config: &Config) -> anyhow::Result<()> {
    let view = load_view(args, config)?;

    let ctx_items = list::list_contexts(&view).context("failed to list contexts")?;

    let picker_items: Vec<PickerItem> = ctx_items
        .iter()
        .map(|item| {
            let is_current = match &args.current {
                Some(name) => item.name == *name,
                None => item.is_current,
            };
            PickerItem {
                name: item.name.clone(),
                is_current,
            }
        })
        .collect();

    let result = picker::pick_inline(&picker_items).context("interactive picker failed")?;

    match result {
        PickerResult::Selected(name) => {
            if args.switch {
                let write_path = resolve_write_path(args, config)?;
                let switch_result = switch::switch_context(&write_path, &name)
                    .with_context(|| format!("failed to switch to context {name:?}"))?;

                if let Some(ref prev) = switch_result.previous {
                    let state =
                        crate::context::state::StateFile::new(&config.cache_dir);
                    if let Err(e) = state.save(prev) {
                        eprintln!("warning: could not save previous context state: {e}");
                    }
                }

                eprintln!("Switched to context \"{}\".", switch_result.current);
            } else {
                println!("{name}");
            }
            Ok(())
        }
        PickerResult::Cancelled => {
            bail!("selection cancelled");
        }
    }
}

fn load_view(args: &PickArgs, config: &Config) -> anyhow::Result<KubeConfigView> {
    if let Some(ref path) = args.kubeconfig {
        KubeConfigView::load(path)
            .with_context(|| format!("failed to load kubeconfig from {}", path.display()))
    } else {
        KubeConfigView::load_merged(&config.kubeconfig_paths)
            .context("failed to load kubeconfig")
    }
}

fn resolve_write_path(args: &PickArgs, config: &Config) -> anyhow::Result<PathBuf> {
    if let Some(ref path) = args.kubeconfig {
        Ok(path.clone())
    } else {
        config
            .kubeconfig_paths
            .first()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("no kubeconfig paths configured"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(input: &[&str]) -> Vec<String> {
        input.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn no_args_returns_defaults() {
        let result = parse_pick_args(&args(&[])).unwrap();
        assert_eq!(result, PickArgs::default());
        assert!(!result.switch);
        assert!(result.kubeconfig.is_none());
        assert!(result.current.is_none());
    }

    #[test]
    fn switch_flag_only() {
        let result = parse_pick_args(&args(&["--switch"])).unwrap();
        assert!(result.switch);
        assert!(result.kubeconfig.is_none());
        assert!(result.current.is_none());
    }

    #[test]
    fn kubeconfig_path() {
        let result =
            parse_pick_args(&args(&["--kubeconfig", "/tmp/kubeconfig"])).unwrap();
        assert!(!result.switch);
        assert_eq!(
            result.kubeconfig,
            Some(PathBuf::from("/tmp/kubeconfig"))
        );
        assert!(result.current.is_none());
    }

    #[test]
    fn current_context_name() {
        let result = parse_pick_args(&args(&["--current", "staging"])).unwrap();
        assert!(!result.switch);
        assert!(result.kubeconfig.is_none());
        assert_eq!(result.current.as_deref(), Some("staging"));
    }

    #[test]
    fn all_flags_combined() {
        let result = parse_pick_args(&args(&[
            "--switch",
            "--kubeconfig",
            "/home/user/.kube/config",
            "--current",
            "production",
        ]))
        .unwrap();

        assert!(result.switch);
        assert_eq!(
            result.kubeconfig,
            Some(PathBuf::from("/home/user/.kube/config"))
        );
        assert_eq!(result.current.as_deref(), Some("production"));
    }

    #[test]
    fn unknown_flag_errors() {
        let err = parse_pick_args(&args(&["--bogus"]))
            .expect_err("should reject unknown flag");
        assert!(
            err.to_string().contains("unknown pick flag"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn kubeconfig_missing_value_errors() {
        let err = parse_pick_args(&args(&["--kubeconfig"]))
            .expect_err("should require a value");
        assert!(
            err.to_string().contains("requires a path"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn current_missing_value_errors() {
        let err = parse_pick_args(&args(&["--current"]))
            .expect_err("should require a value");
        assert!(
            err.to_string().contains("requires a context name"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn flags_in_different_order() {
        let result = parse_pick_args(&args(&[
            "--current",
            "dev",
            "--switch",
            "--kubeconfig",
            "/etc/kube/config",
        ]))
        .unwrap();

        assert!(result.switch);
        assert_eq!(
            result.kubeconfig,
            Some(PathBuf::from("/etc/kube/config"))
        );
        assert_eq!(result.current.as_deref(), Some("dev"));
    }
}
