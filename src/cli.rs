// Rust guideline compliant 2026-02-21
//! CLI argument parsing and application-level orchestration.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context as _};

use crate::context::state::StateFile;
use crate::context::{current, list, mutate, switch};
use crate::dispatch::ToolMode;
use crate::integration::k9s::{self, PickArgs};
use crate::kubeconfig::KubeConfigView;
use crate::picker::{self, PickerItem, PickerResult};

const VERSION: &str = env!("CARGO_PKG_VERSION");

const HELP_TEXT: &str = "\
USAGE:
  kubectx                   : list the contexts (interactive picker when a TTY)
  kubectx <NAME>            : switch to context <NAME>
  kubectx -                 : switch to the previous context
  kubectx -c, --current     : show the current context name
  kubectx -d, --delete NAME : delete context NAME (or '.' for current)
  kubectx <NEW>=<OLD>       : rename context <OLD> to <NEW>
  kubectx -u, --unset       : unset the current context
  kubectx --fzf             : use external fzf for interactive selection
  kubectx pick [--switch] [--kubeconfig <path>] [--current <ctx>]
                            : interactive picker (k9s plugin)
  kubectx -h, --help        : show this help
  kubectx -V, --version     : show version";

/// Parsed command to execute.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum Command {
    List,
    Switch { target: String },
    SwapPrevious,
    Current,
    Delete { target: String },
    Rename { old: String, new_name: String },
    Unset,
    InteractiveFzf,
    Pick(PickArgs),
}

impl Command {
    /// Whether this command would modify the active context or kubeconfig.
    const fn modifies_context(&self) -> bool {
        matches!(
            self,
            Self::Switch { .. }
                | Self::SwapPrevious
                | Self::Delete { .. }
                | Self::Rename { .. }
                | Self::Unset
                | Self::InteractiveFzf
                | Self::Pick(_)
        )
    }
}

/// Sentinel returned by `parse_args` when the user requested `--help`
/// or `--version` and the output was already printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ParseResult {
    Run(Command),
    Exit,
}

/// Parse CLI arguments (everything after argv0) into a [`Command`].
///
/// Prints help or version to stdout when requested and returns
/// [`ParseResult::Exit`] so the caller can exit cleanly.
pub(crate) fn parse_args(args: &[String]) -> anyhow::Result<ParseResult> {
    if args.is_empty() {
        return Ok(ParseResult::Run(Command::List));
    }

    let first = args[0].as_str();

    match first {
        "-h" | "--help" => {
            println!("{HELP_TEXT}");
            Ok(ParseResult::Exit)
        }
        "-V" | "--version" => {
            println!("kubectx {VERSION}");
            Ok(ParseResult::Exit)
        }
        "-c" | "--current" => {
            ensure_no_extra(args, first)?;
            Ok(ParseResult::Run(Command::Current))
        }
        "-u" | "--unset" => {
            ensure_no_extra(args, first)?;
            Ok(ParseResult::Run(Command::Unset))
        }
        "-d" | "--delete" => {
            let target = args
                .get(1)
                .map(String::as_str)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| anyhow::anyhow!("{first} requires a context name"))?;

            if args.len() > 2 {
                bail!("{first} takes exactly one argument");
            }

            Ok(ParseResult::Run(Command::Delete {
                target: target.to_owned(),
            }))
        }
        "--fzf" => {
            ensure_no_extra(args, "--fzf")?;
            Ok(ParseResult::Run(Command::InteractiveFzf))
        }
        "-" => {
            ensure_no_extra(args, "-")?;
            Ok(ParseResult::Run(Command::SwapPrevious))
        }
        "pick" => {
            let pick_args = k9s::parse_pick_args(&args[1..])?;
            Ok(ParseResult::Run(Command::Pick(pick_args)))
        }
        arg if arg.starts_with('-') => {
            bail!("unknown flag: {arg}\nRun with --help for usage information")
        }
        positional => {
            if args.len() > 1 {
                bail!("unexpected extra arguments after {positional:?}");
            }

            if let Some(eq_pos) = positional.find('=') {
                let new_name = &positional[..eq_pos];
                let old = &positional[eq_pos + 1..];

                if new_name.is_empty() || old.is_empty() {
                    bail!("rename syntax is <NEW>=<OLD>, both sides must be non-empty");
                }

                Ok(ParseResult::Run(Command::Rename {
                    old: old.to_owned(),
                    new_name: new_name.to_owned(),
                }))
            } else {
                Ok(ParseResult::Run(Command::Switch {
                    target: positional.to_owned(),
                }))
            }
        }
    }
}

/// Reject extra arguments after a flag that takes none.
fn ensure_no_extra(args: &[String], flag: &str) -> anyhow::Result<()> {
    if args.len() > 1 {
        bail!("{flag} does not accept additional arguments");
    }
    Ok(())
}

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
pub fn execute(mode: ToolMode, config: &Config) -> anyhow::Result<()> {
    if mode == ToolMode::Kubens {
        eprintln!("kubens mode not yet implemented");
        return Ok(());
    }

    let user_args: Vec<String> = std::env::args().skip(1).collect();
    let result = parse_args(&user_args)?;

    let command = match result {
        ParseResult::Run(cmd) => cmd,
        ParseResult::Exit => return Ok(()),
    };

    dispatch_command(command, config)
}

fn dispatch_command(command: Command, config: &Config) -> anyhow::Result<()> {
    if config.isolated_shell && command.modifies_context() {
        bail!(
            "context switching is disabled in this shell \
             (KUBECTX_ISOLATED_SHELL=1)"
        );
    }

    match command {
        Command::List => cmd_list_or_interactive(config),
        Command::Current => cmd_current(config),
        Command::Switch { target } => cmd_switch(config, &target),
        Command::SwapPrevious => cmd_swap_previous(config),
        Command::Delete { target } => cmd_delete(config, &target),
        Command::Rename { old, new_name } => cmd_rename(config, &old, &new_name),
        Command::Unset => cmd_unset(config),
        Command::InteractiveFzf => cmd_interactive(config, true),
        Command::Pick(ref pick_args) => k9s::execute_pick(pick_args, config),
    }
}

fn load_merged_view(config: &Config) -> anyhow::Result<KubeConfigView> {
    KubeConfigView::load_merged(&config.kubeconfig_paths)
        .context("failed to load kubeconfig")
}

fn cmd_list_or_interactive(config: &Config) -> anyhow::Result<()> {
    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stdout());
    let ignore_fzf = std::env::var_os("KUBECTX_IGNORE_FZF").is_some();

    if is_tty && !ignore_fzf && !config.isolated_shell {
        return cmd_interactive(config, false);
    }

    let view = load_merged_view(config)?;
    let items = list::list_contexts(&view).context("failed to list contexts")?;

    for item in &items {
        if item.is_current {
            println!("* {}", item.name);
        } else {
            println!("  {}", item.name);
        }
    }

    Ok(())
}

fn cmd_interactive(config: &Config, use_fzf: bool) -> anyhow::Result<()> {
    let view = load_merged_view(config)?;
    let ctx_items = list::list_contexts(&view).context("failed to list contexts")?;

    let picker_items: Vec<PickerItem> = ctx_items
        .iter()
        .map(|i| PickerItem {
            name: i.name.clone(),
            is_current: i.is_current,
        })
        .collect();

    let result = if use_fzf {
        picker::fzf::pick_fzf(&picker_items).context("fzf picker failed")?
    } else {
        picker::pick_inline(&picker_items).context("interactive picker failed")?
    };

    match result {
        PickerResult::Selected(name) => cmd_switch(config, &name),
        PickerResult::Cancelled => Ok(()),
    }
}

fn cmd_current(config: &Config) -> anyhow::Result<()> {
    let view = load_merged_view(config)?;
    let name = current::current_context(&view).context("no current context is set")?;
    println!("{name}");
    Ok(())
}

fn cmd_switch(config: &Config, target: &str) -> anyhow::Result<()> {
    let write_path = primary_kubeconfig(config)?;

    let result = switch::switch_context(&write_path, target)
        .with_context(|| format!("failed to switch to context {target:?}"))?;

    if let Some(ref prev) = result.previous {
        let state = StateFile::new(&config.cache_dir);
        if let Err(e) = state.save(prev) {
            eprintln!("warning: could not save previous context state: {e}");
        }
    }

    eprintln!("Switched to context \"{}\".", result.current);
    Ok(())
}

fn cmd_swap_previous(config: &Config) -> anyhow::Result<()> {
    let state = StateFile::new(&config.cache_dir);
    let previous = state
        .load()
        .context("failed to read state file")?
        .ok_or_else(|| anyhow::anyhow!("no previous context found"))?;

    cmd_switch(config, &previous)
}

fn cmd_delete(config: &Config, target: &str) -> anyhow::Result<()> {
    let write_path = primary_kubeconfig(config)?;

    let result = if target == "." {
        mutate::delete_current_context(write_path)
            .context("failed to delete current context")?
    } else {
        mutate::delete_context(write_path, target)
            .with_context(|| format!("failed to delete context {target:?}"))?
    };

    if result.was_current {
        eprintln!(
            "Deleted context \"{}\" (was current, now unset).",
            result.deleted
        );
    } else {
        eprintln!("Deleted context \"{}\".", result.deleted);
    }
    Ok(())
}

fn cmd_rename(config: &Config, old: &str, new_name: &str) -> anyhow::Result<()> {
    let write_path = primary_kubeconfig(config)?;
    let result = mutate::rename_context(&write_path, old, new_name)
        .with_context(|| format!("failed to rename context {old:?} to {new_name:?}"))?;

    eprintln!(
        "Context \"{}\" renamed to \"{}\".",
        result.old_name, result.new_name
    );
    Ok(())
}

fn cmd_unset(config: &Config) -> anyhow::Result<()> {
    let write_path = primary_kubeconfig(config)?;
    let result = mutate::unset_context(&write_path).context("failed to unset current context")?;

    match result.previous {
        Some(prev) => eprintln!("Active context unset (was \"{prev}\")."),
        None => eprintln!("Already no active context."),
    }
    Ok(())
}

/// Return the first kubeconfig path, used for write operations.
fn primary_kubeconfig(config: &Config) -> anyhow::Result<&Path> {
    config
        .kubeconfig_paths
        .first()
        .map(PathBuf::as_path)
        .ok_or_else(|| anyhow::anyhow!("no kubeconfig paths configured"))
}

fn resolve_kubeconfig_paths() -> Vec<PathBuf> {
    if let Ok(val) = std::env::var("KUBECONFIG") {
        val.split(':')
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .collect()
    } else {
        let home = directories::BaseDirs::new()
            .map_or_else(|| PathBuf::from("~"), |d| d.home_dir().to_path_buf());
        vec![home.join(".kube").join("config")]
    }
}

fn resolve_cache_dir() -> PathBuf {
    if let Ok(val) = std::env::var("XDG_CACHE_HOME") {
        PathBuf::from(val)
    } else {
        directories::BaseDirs::new()
            .map_or_else(|| PathBuf::from("."), |d| d.home_dir().join(".kube"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(input: &[&str]) -> Vec<String> {
        input.iter().map(|s| (*s).to_string()).collect()
    }

    fn parse(input: &[&str]) -> anyhow::Result<ParseResult> {
        parse_args(&args(input))
    }

    fn expect_cmd(input: &[&str]) -> Command {
        match parse(input).expect("should parse successfully") {
            ParseResult::Run(cmd) => cmd,
            ParseResult::Exit => panic!("expected a Command, got Exit"),
        }
    }

    fn expect_exit(input: &[&str]) {
        match parse(input).expect("should parse successfully") {
            ParseResult::Exit => {}
            ParseResult::Run(cmd) => panic!("expected Exit, got {cmd:?}"),
        }
    }

    fn expect_err(input: &[&str]) -> String {
        parse(input)
            .expect_err("should fail to parse")
            .to_string()
    }

    // -- No args -> List --

    #[test]
    fn no_args_produces_list() {
        assert_eq!(expect_cmd(&[]), Command::List);
    }

    // -- Current context --

    #[test]
    fn flag_c_produces_current() {
        assert_eq!(expect_cmd(&["-c"]), Command::Current);
    }

    #[test]
    fn flag_current_produces_current() {
        assert_eq!(expect_cmd(&["--current"]), Command::Current);
    }

    #[test]
    fn current_rejects_extra_args() {
        let err = expect_err(&["-c", "foo"]);
        assert!(err.contains("does not accept additional arguments"), "{err}");
    }

    // -- Unset --

    #[test]
    fn flag_u_produces_unset() {
        assert_eq!(expect_cmd(&["-u"]), Command::Unset);
    }

    #[test]
    fn flag_unset_produces_unset() {
        assert_eq!(expect_cmd(&["--unset"]), Command::Unset);
    }

    #[test]
    fn unset_rejects_extra_args() {
        let err = expect_err(&["-u", "foo"]);
        assert!(err.contains("does not accept additional arguments"), "{err}");
    }

    // -- Delete --

    #[test]
    fn flag_d_with_name_produces_delete() {
        assert_eq!(
            expect_cmd(&["-d", "staging"]),
            Command::Delete {
                target: "staging".to_owned()
            }
        );
    }

    #[test]
    fn flag_delete_with_dot_produces_delete() {
        assert_eq!(
            expect_cmd(&["--delete", "."]),
            Command::Delete {
                target: ".".to_owned()
            }
        );
    }

    #[test]
    fn delete_without_name_is_error() {
        let err = expect_err(&["-d"]);
        assert!(err.contains("requires a context name"), "{err}");
    }

    #[test]
    fn delete_with_extra_args_is_error() {
        let err = expect_err(&["-d", "a", "b"]);
        assert!(err.contains("takes exactly one argument"), "{err}");
    }

    // -- Swap previous --

    #[test]
    fn dash_produces_swap_previous() {
        assert_eq!(expect_cmd(&["-"]), Command::SwapPrevious);
    }

    #[test]
    fn dash_rejects_extra_args() {
        let err = expect_err(&["-", "foo"]);
        assert!(err.contains("does not accept additional arguments"), "{err}");
    }

    // -- Switch --

    #[test]
    fn bare_name_produces_switch() {
        assert_eq!(
            expect_cmd(&["production"]),
            Command::Switch {
                target: "production".to_owned()
            }
        );
    }

    #[test]
    fn switch_rejects_extra_args() {
        let err = expect_err(&["a", "b"]);
        assert!(err.contains("unexpected extra arguments"), "{err}");
    }

    // -- Rename --

    #[test]
    fn equals_syntax_produces_rename() {
        assert_eq!(
            expect_cmd(&["new-ctx=old-ctx"]),
            Command::Rename {
                old: "old-ctx".to_owned(),
                new_name: "new-ctx".to_owned(),
            }
        );
    }

    #[test]
    fn rename_with_empty_left_is_error() {
        let err = expect_err(&["=old"]);
        assert!(err.contains("both sides must be non-empty"), "{err}");
    }

    #[test]
    fn rename_with_empty_right_is_error() {
        let err = expect_err(&["new="]);
        assert!(err.contains("both sides must be non-empty"), "{err}");
    }

    // -- Help and version --

    #[test]
    fn help_short() {
        expect_exit(&["-h"]);
    }

    #[test]
    fn help_long() {
        expect_exit(&["--help"]);
    }

    #[test]
    fn version_short() {
        expect_exit(&["-V"]);
    }

    #[test]
    fn version_long() {
        expect_exit(&["--version"]);
    }

    // -- Fzf flag --

    #[test]
    fn fzf_flag_produces_interactive_fzf() {
        assert_eq!(expect_cmd(&["--fzf"]), Command::InteractiveFzf);
    }

    #[test]
    fn fzf_rejects_extra_args() {
        let err = expect_err(&["--fzf", "foo"]);
        assert!(err.contains("does not accept additional arguments"), "{err}");
    }

    // -- Unknown flags --

    #[test]
    fn unknown_flag_is_error() {
        let err = expect_err(&["--foobar"]);
        assert!(err.contains("unknown flag"), "{err}");
    }

    #[test]
    fn unknown_short_flag_is_error() {
        let err = expect_err(&["-x"]);
        assert!(err.contains("unknown flag"), "{err}");
    }

    // -- Pick subcommand --

    #[test]
    fn pick_alone_produces_default_pick_args() {
        let cmd = expect_cmd(&["pick"]);
        assert_eq!(
            cmd,
            Command::Pick(PickArgs {
                switch: false,
                kubeconfig: None,
                current: None,
            })
        );
    }

    #[test]
    fn pick_with_switch_flag() {
        let cmd = expect_cmd(&["pick", "--switch"]);
        assert_eq!(
            cmd,
            Command::Pick(PickArgs {
                switch: true,
                kubeconfig: None,
                current: None,
            })
        );
    }

    #[test]
    fn pick_with_all_flags() {
        let cmd = expect_cmd(&["pick", "--switch", "--kubeconfig", "/tmp/cfg", "--current", "dev"]);
        assert_eq!(
            cmd,
            Command::Pick(PickArgs {
                switch: true,
                kubeconfig: Some(std::path::PathBuf::from("/tmp/cfg")),
                current: Some("dev".to_owned()),
            })
        );
    }

    #[test]
    fn pick_with_unknown_flag_errors() {
        let err = expect_err(&["pick", "--bogus"]);
        assert!(err.contains("unknown pick flag"), "{err}");
    }
}
