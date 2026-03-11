//! CLI argument parsing and application-level orchestration.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, bail};
use crossterm::style::Stylize;

use crate::context::state::StateFile;
use crate::context::{current, list, mutate, switch};
use crate::dispatch::ToolMode;
use crate::integration::k9s::{self, PickArgs};
use crate::kubeconfig::KubeConfigView;
use crate::namespace::current::current_namespace;
use crate::namespace::state::NsStateFile;
use crate::picker::{self, PickerItem, PickerResult};

const VERSION: &str = env!("CARGO_PKG_VERSION");

const KUBENS_HELP_TEXT: &str = "\
USAGE:
  kubens                    : list the namespaces (interactive picker when a TTY)
  kubens <NAME>             : switch to namespace <NAME>
  kubens -                  : switch to the previous namespace
  kubens -c, --current      : show the current namespace
  kubens -u, --unset        : reset namespace to default
  kubens -f, --force <NAME> : switch without checking namespace exists
  kubens --raw              : list namespace names (no prefix, no color)
  kubens --fzf              : use external fzf for interactive selection
  kubens --completion SHELL : output shell completion (bash, zsh, fish)
  kubens -h, --help         : show this help
  kubens -V, --version      : show version";

const HELP_TEXT: &str = "\
USAGE:
  kubectx                   : list the contexts (interactive picker when a TTY)
  kubectx <NAME>            : switch to context <NAME>
  kubectx -                 : switch to the previous context
  kubectx -c, --current     : show the current context name
  kubectx -d, --delete NAME [NAME...] : delete context(s) ('.' for current)
  kubectx <NEW>=<OLD>       : rename context <OLD> to <NEW> ('.' for current)
  kubectx -u, --unset       : unset the current context
  kubectx --raw             : list context names (no prefix, no color)
  kubectx --fzf             : use external fzf for interactive selection
  kubectx ns <args>          : namespace mode (see kubens --help)
  kubectx pick [--switch] [--kubeconfig <path>] [--current <ctx>]
                            : interactive picker (k9s plugin)
  kubectx --completion SHELL : output shell completion (bash, zsh, fish)
  kubectx -h, --help        : show this help
  kubectx -V, --version     : show version";

/// Parsed command to execute.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum Command {
    List,
    ListRaw,
    Switch { target: String },
    SwapPrevious,
    Current,
    Delete { targets: Vec<String> },
    Rename { old: String, new_name: String },
    Unset,
    InteractiveFzf,
    Pick(PickArgs),
    Completion { shell: crate::completion::Shell },
    Ns(NsCommand),
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

/// Sentinel returned by argument parsers when the user requested `--help`
/// or `--version` and the output was already printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ParseResult<T> {
    Run(T),
    Exit,
}

/// Parsed namespace command to execute.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum NsCommand {
    List,
    ListRaw,
    Switch { target: String, force: bool },
    SwapPrevious,
    Current,
    Unset,
    InteractiveFzf,
    Completion { shell: crate::completion::Shell },
}

/// Parse CLI arguments (everything after argv0) into a [`Command`].
///
/// Prints help or version to stdout when requested and returns
/// [`ParseResult::Exit`] so the caller can exit cleanly.
pub(crate) fn parse_args(args: &[String]) -> anyhow::Result<ParseResult<Command>> {
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
            let targets: Vec<String> = args[1..]
                .iter()
                .filter(|s| !s.is_empty())
                .cloned()
                .collect();

            if targets.is_empty() {
                bail!("{first} requires at least one context name");
            }

            Ok(ParseResult::Run(Command::Delete { targets }))
        }
        "--raw" => {
            ensure_no_extra(args, "--raw")?;
            Ok(ParseResult::Run(Command::ListRaw))
        }
        "--fzf" => {
            ensure_no_extra(args, "--fzf")?;
            Ok(ParseResult::Run(Command::InteractiveFzf))
        }
        "-" => {
            ensure_no_extra(args, "-")?;
            Ok(ParseResult::Run(Command::SwapPrevious))
        }
        "--completion" => {
            let shell = parse_completion_shell(args)?;
            Ok(ParseResult::Run(Command::Completion { shell }))
        }
        "pick" => {
            let pick_args = k9s::parse_pick_args(&args[1..])?;
            Ok(ParseResult::Run(Command::Pick(pick_args)))
        }
        "ns" => match parse_ns_args(&args[1..])? {
            ParseResult::Run(ns_cmd) => Ok(ParseResult::Run(Command::Ns(ns_cmd))),
            ParseResult::Exit => Ok(ParseResult::Exit),
        },
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

/// Parse `--completion <shell>` from args[0..], returning the shell variant.
fn parse_completion_shell(args: &[String]) -> anyhow::Result<crate::completion::Shell> {
    let shell_name = args
        .get(1)
        .map(String::as_str)
        .ok_or_else(|| anyhow::anyhow!("--completion requires a shell name (bash, zsh, fish)"))?;
    if args.len() > 2 {
        bail!("--completion takes exactly one argument");
    }
    shell_name
        .parse::<crate::completion::Shell>()
        .map_err(|e| anyhow::anyhow!("{e}"))
}

/// Reject extra arguments after a flag that takes none.
fn ensure_no_extra(args: &[String], flag: &str) -> anyhow::Result<()> {
    if args.len() > 1 {
        bail!("{flag} does not accept additional arguments");
    }
    Ok(())
}

/// Parse kubens CLI arguments into a [`NsCommand`].
///
/// Prints help or version to stdout when requested and returns
/// [`ParseResult::Exit`] so the caller can exit cleanly.
pub(crate) fn parse_ns_args(args: &[String]) -> anyhow::Result<ParseResult<NsCommand>> {
    if args.is_empty() {
        return Ok(ParseResult::Run(NsCommand::List));
    }

    let first = args[0].as_str();

    match first {
        "-h" | "--help" => {
            println!("{KUBENS_HELP_TEXT}");
            Ok(ParseResult::Exit)
        }
        "-V" | "--version" => {
            println!("kubens {VERSION}");
            Ok(ParseResult::Exit)
        }
        "-c" | "--current" => {
            ensure_no_extra(args, first)?;
            Ok(ParseResult::Run(NsCommand::Current))
        }
        "-u" | "--unset" => {
            ensure_no_extra(args, first)?;
            Ok(ParseResult::Run(NsCommand::Unset))
        }
        "--raw" => {
            ensure_no_extra(args, "--raw")?;
            Ok(ParseResult::Run(NsCommand::ListRaw))
        }
        "--fzf" => {
            ensure_no_extra(args, "--fzf")?;
            Ok(ParseResult::Run(NsCommand::InteractiveFzf))
        }
        "-" => {
            ensure_no_extra(args, "-")?;
            Ok(ParseResult::Run(NsCommand::SwapPrevious))
        }
        "--completion" => {
            let shell = parse_completion_shell(args)?;
            Ok(ParseResult::Run(NsCommand::Completion { shell }))
        }
        "-f" | "--force" => {
            let target = args
                .get(1)
                .map(String::as_str)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| anyhow::anyhow!("{first} requires a namespace name"))?;
            if args.len() > 2 {
                bail!("{first} takes exactly one argument");
            }
            Ok(ParseResult::Run(NsCommand::Switch {
                target: target.to_owned(),
                force: true,
            }))
        }
        arg if arg.starts_with('-') => {
            bail!("unknown flag: {arg}\nRun with --help for usage information")
        }
        positional => {
            // Check for trailing -f / --force after the name
            let force = match args.get(1).map(String::as_str) {
                Some("-f" | "--force") => true,
                Some(other) => {
                    bail!("unexpected extra argument after {positional:?}: {other:?}");
                }
                None => false,
            };
            if args.len() > 2 {
                bail!("unexpected extra arguments after {positional:?}");
            }
            Ok(ParseResult::Run(NsCommand::Switch {
                target: positional.to_owned(),
                force,
            }))
        }
    }
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
            isolated_shell: std::env::var_os("KUBECTX_ISOLATED_SHELL").is_some_and(|v| v == "1"),
        }
    }
}

/// Execute the resolved command in the given tool mode.
///
/// # Errors
///
/// Returns an error if command execution fails.
pub fn execute(mode: ToolMode, config: &Config) -> anyhow::Result<()> {
    let user_args: Vec<String> = std::env::args().skip(1).collect();

    let benchmark = user_args.iter().any(|a| a == "--benchmark");
    let filtered_args: Vec<String> = if benchmark {
        user_args
            .into_iter()
            .filter(|a| a != "--benchmark")
            .collect()
    } else {
        user_args
    };

    match mode {
        ToolMode::Kubectx => {
            let result = parse_args(&filtered_args)?;
            let command = match result {
                ParseResult::Run(cmd) => cmd,
                ParseResult::Exit => return Ok(()),
            };
            if benchmark {
                run_benchmarked(|| dispatch_command(command, config))
            } else {
                dispatch_command(command, config)
            }
        }
        ToolMode::Kubens => {
            let result = parse_ns_args(&filtered_args)?;
            let command = match result {
                ParseResult::Run(cmd) => cmd,
                ParseResult::Exit => return Ok(()),
            };
            if benchmark {
                run_benchmarked(|| dispatch_ns_command(command, config))
            } else {
                dispatch_ns_command(command, config)
            }
        }
    }
}

fn run_benchmarked(f: impl FnOnce() -> anyhow::Result<()>) -> anyhow::Result<()> {
    let start = std::time::Instant::now();
    let result = f();
    let elapsed = start.elapsed();
    eprintln!("[benchmark] {elapsed:?}");
    result
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
        Command::ListRaw => cmd_list_raw(config),
        Command::Current => cmd_current(config),
        Command::Switch { target } => cmd_switch(config, &target),
        Command::SwapPrevious => cmd_swap_previous(config),
        Command::Delete { ref targets } => cmd_delete(config, targets),
        Command::Rename { old, new_name } => cmd_rename(config, &old, &new_name),
        Command::Unset => cmd_unset(config),
        Command::InteractiveFzf => cmd_interactive(config, true),
        Command::Pick(ref pick_args) => k9s::execute_pick(pick_args, config),
        Command::Completion { shell } => {
            print!("{}", crate::completion::generate(shell));
            Ok(())
        }
        Command::Ns(ns_cmd) => dispatch_ns_command(ns_cmd, config),
    }
}

fn load_merged_view(config: &Config) -> anyhow::Result<KubeConfigView> {
    KubeConfigView::load_merged(&config.kubeconfig_paths).context("failed to load kubeconfig")
}

/// Load the merged kubeconfig, returning `None` if the file does not exist.
fn load_merged_or_empty(config: &Config) -> anyhow::Result<Option<KubeConfigView>> {
    match KubeConfigView::load_merged(&config.kubeconfig_paths) {
        Ok(v) => Ok(Some(v)),
        Err(e) if e.is_not_found() => Ok(None),
        Err(e) => Err(e).context("failed to load kubeconfig"),
    }
}

fn cmd_list_or_interactive(config: &Config) -> anyhow::Result<()> {
    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stdout());
    let ignore_fzf = std::env::var_os("KUBECTX_IGNORE_FZF").is_some();

    if is_tty && !ignore_fzf && !config.isolated_shell {
        return cmd_interactive(config, false);
    }

    let Some(view) = load_merged_or_empty(config)? else {
        return Ok(());
    };

    let items = list::list_contexts(&view).context("failed to list contexts")?;
    let use_color = config.force_color || (is_tty && !config.no_color);

    for item in &items {
        print_list_item(&item.name, item.is_current, use_color);
    }

    Ok(())
}

fn print_list_item(name: &str, is_current: bool, use_color: bool) {
    if is_current {
        if use_color {
            println!("{}", format!("* {name}").green().bold());
        } else {
            println!("* {name}");
        }
    } else {
        println!("  {name}");
    }
}

fn cmd_list_raw(config: &Config) -> anyhow::Result<()> {
    let Some(view) = load_merged_or_empty(config)? else {
        return Ok(());
    };

    let items = list::list_contexts(&view).context("failed to list contexts")?;

    for item in &items {
        println!("{}", item.name);
    }

    Ok(())
}

fn cmd_interactive(config: &Config, use_fzf: bool) -> anyhow::Result<()> {
    let view = load_merged_view(config)?;
    let ctx_items = list::list_contexts(&view).context("failed to list contexts")?;

    let picker_items: Vec<PickerItem> = ctx_items.into_iter().map(PickerItem::from).collect();

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

    let result = switch::switch_context(write_path, target)
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

fn cmd_delete(config: &Config, targets: &[String]) -> anyhow::Result<()> {
    let write_path = primary_kubeconfig(config)?;

    for target in targets {
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
    }
    Ok(())
}

fn cmd_rename(config: &Config, old: &str, new_name: &str) -> anyhow::Result<()> {
    let write_path = primary_kubeconfig(config)?;

    let resolved_old;
    let old = if old == "." {
        let view = load_merged_view(config)?;
        resolved_old = view
            .current_context()
            .ok_or_else(|| anyhow::anyhow!("cannot resolve '.' — no current context set"))?
            .to_owned();
        resolved_old.as_str()
    } else {
        old
    };

    let result = mutate::rename_context(write_path, old, new_name)
        .with_context(|| format!("failed to rename context {old:?} to {new_name:?}"))?;

    eprintln!(
        "Context \"{}\" renamed to \"{}\".",
        result.old_name, result.new_name
    );
    Ok(())
}

fn cmd_unset(config: &Config) -> anyhow::Result<()> {
    let write_path = primary_kubeconfig(config)?;
    let result = mutate::unset_context(write_path).context("failed to unset current context")?;

    match result.previous {
        Some(prev) => eprintln!("Active context unset (was \"{prev}\")."),
        None => eprintln!("Already no active context."),
    }
    Ok(())
}

// -- kubens command dispatch and handlers --

fn dispatch_ns_command(command: NsCommand, config: &Config) -> anyhow::Result<()> {
    match command {
        NsCommand::List => ns_cmd_list_or_interactive(config),
        NsCommand::ListRaw => ns_cmd_list_raw(config),
        NsCommand::Current => ns_cmd_current(config),
        NsCommand::Switch { target, force } => ns_cmd_switch(config, &target, force),
        NsCommand::SwapPrevious => ns_cmd_swap_previous(config),
        NsCommand::Unset => ns_cmd_unset(config),
        NsCommand::InteractiveFzf => ns_cmd_interactive(config, true),
        NsCommand::Completion { shell } => {
            print!("{}", crate::completion::generate_kubens(shell));
            Ok(())
        }
    }
}

fn ns_cmd_current(config: &Config) -> anyhow::Result<()> {
    let view = load_merged_view(config)?;
    let ns = current_namespace(&view).context("failed to resolve current namespace")?;
    println!("{ns}");
    Ok(())
}

fn ns_cmd_switch(config: &Config, target: &str, force: bool) -> anyhow::Result<()> {
    let write_path = primary_kubeconfig(config)?;

    if !force {
        crate::namespace::list::namespace_exists(write_path, target)
            .with_context(|| format!("failed to verify namespace {target:?}"))?;
    }

    let result = crate::namespace::switch::switch_namespace(write_path, target)
        .with_context(|| format!("failed to switch to namespace {target:?}"))?;

    let ns_state = NsStateFile::new(&config.cache_dir, &result.context);
    if let Err(e) = ns_state.save(&result.previous) {
        eprintln!("warning: could not save previous namespace state: {e}");
    }

    eprintln!("Switched to namespace \"{target}\".");
    Ok(())
}

fn ns_cmd_swap_previous(config: &Config) -> anyhow::Result<()> {
    let write_path = primary_kubeconfig(config)?;
    let view = KubeConfigView::load(write_path).context("failed to load kubeconfig")?;
    let ctx_name = view
        .current_context()
        .ok_or_else(|| anyhow::anyhow!("no current context set"))?;

    let ns_state = NsStateFile::new(&config.cache_dir, ctx_name);
    let previous = ns_state
        .load()
        .context("failed to read namespace state file")?
        .ok_or_else(|| anyhow::anyhow!("no previous namespace found"))?;

    ns_cmd_switch(config, &previous, true)
}

fn ns_cmd_unset(config: &Config) -> anyhow::Result<()> {
    let write_path = primary_kubeconfig(config)?;
    let result = crate::namespace::switch::unset_namespace(write_path)
        .context("failed to unset namespace")?;
    eprintln!("Active namespace unset (was \"{}\").", result.previous);
    Ok(())
}

fn ns_cmd_list_or_interactive(config: &Config) -> anyhow::Result<()> {
    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stdout());
    let ignore_fzf = std::env::var_os("KUBECTX_IGNORE_FZF").is_some();

    if is_tty && !ignore_fzf {
        return ns_cmd_interactive(config, false);
    }

    let view = load_merged_view(config)?;
    let current_ns = current_namespace(&view).unwrap_or_default();

    let write_path = primary_kubeconfig(config)?;
    let namespaces =
        crate::namespace::list::list_namespaces(write_path).context("failed to list namespaces")?;

    let use_color = config.force_color || (is_tty && !config.no_color);

    for ns in &namespaces {
        print_list_item(ns, ns == &current_ns, use_color);
    }

    Ok(())
}

fn ns_cmd_list_raw(config: &Config) -> anyhow::Result<()> {
    let write_path = primary_kubeconfig(config)?;
    let namespaces =
        crate::namespace::list::list_namespaces(write_path).context("failed to list namespaces")?;

    for ns in &namespaces {
        println!("{ns}");
    }

    Ok(())
}

fn ns_cmd_interactive(config: &Config, use_fzf: bool) -> anyhow::Result<()> {
    let view = load_merged_view(config)?;
    let current_ns = current_namespace(&view).unwrap_or_default();

    let write_path = primary_kubeconfig(config)?;
    let namespaces =
        crate::namespace::list::list_namespaces(write_path).context("failed to list namespaces")?;

    let picker_items: Vec<PickerItem> = namespaces
        .iter()
        .map(|ns| PickerItem {
            name: ns.clone(),
            is_current: ns == &current_ns,
            meta: None,
        })
        .collect();

    let result = if use_fzf {
        picker::fzf::pick_fzf(&picker_items).context("fzf picker failed")?
    } else {
        picker::pick_inline(&picker_items).context("interactive picker failed")?
    };

    match result {
        PickerResult::Selected(name) => ns_cmd_switch(config, &name, true),
        PickerResult::Cancelled => Ok(()),
    }
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
        std::env::split_paths(&val)
            .filter(|p| !p.as_os_str().is_empty())
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

    fn parse(input: &[&str]) -> anyhow::Result<ParseResult<Command>> {
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
        parse(input).expect_err("should fail to parse").to_string()
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
        assert!(
            err.contains("does not accept additional arguments"),
            "{err}"
        );
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
        assert!(
            err.contains("does not accept additional arguments"),
            "{err}"
        );
    }

    // -- Delete --

    #[test]
    fn flag_d_with_name_produces_delete() {
        assert_eq!(
            expect_cmd(&["-d", "staging"]),
            Command::Delete {
                targets: vec!["staging".to_owned()]
            }
        );
    }

    #[test]
    fn flag_delete_with_dot_produces_delete() {
        assert_eq!(
            expect_cmd(&["--delete", "."]),
            Command::Delete {
                targets: vec![".".to_owned()]
            }
        );
    }

    #[test]
    fn delete_multiple_targets() {
        assert_eq!(
            expect_cmd(&["-d", "staging", "dev", "test"]),
            Command::Delete {
                targets: vec!["staging".to_owned(), "dev".to_owned(), "test".to_owned(),]
            }
        );
    }

    #[test]
    fn delete_without_name_is_error() {
        let err = expect_err(&["-d"]);
        assert!(err.contains("requires at least one context name"), "{err}");
    }

    // -- Swap previous --

    #[test]
    fn dash_produces_swap_previous() {
        assert_eq!(expect_cmd(&["-"]), Command::SwapPrevious);
    }

    #[test]
    fn dash_rejects_extra_args() {
        let err = expect_err(&["-", "foo"]);
        assert!(
            err.contains("does not accept additional arguments"),
            "{err}"
        );
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
        assert!(
            err.contains("does not accept additional arguments"),
            "{err}"
        );
    }

    // -- Raw flag --

    #[test]
    fn raw_flag_produces_list_raw() {
        assert_eq!(expect_cmd(&["--raw"]), Command::ListRaw);
    }

    #[test]
    fn raw_rejects_extra_args() {
        let err = expect_err(&["--raw", "foo"]);
        assert!(
            err.contains("does not accept additional arguments"),
            "{err}"
        );
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

    #[test]
    fn benchmark_flag_unknown_to_parser() {
        let err = expect_err(&["--benchmark"]);
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
        let cmd = expect_cmd(&[
            "pick",
            "--switch",
            "--kubeconfig",
            "/tmp/cfg",
            "--current",
            "dev",
        ]);
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

    // -- Ns subcommand --

    #[test]
    fn ns_subcommand_no_args_produces_list() {
        assert_eq!(expect_cmd(&["ns"]), Command::Ns(NsCommand::List));
    }

    #[test]
    fn ns_subcommand_switch_target() {
        assert_eq!(
            expect_cmd(&["ns", "kube-system"]),
            Command::Ns(NsCommand::Switch {
                target: "kube-system".to_owned(),
                force: false,
            })
        );
    }

    #[test]
    fn ns_subcommand_current() {
        assert_eq!(expect_cmd(&["ns", "-c"]), Command::Ns(NsCommand::Current));
    }

    #[test]
    fn ns_subcommand_help_produces_exit() {
        expect_exit(&["ns", "--help"]);
    }

    #[test]
    fn ns_subcommand_force_switch() {
        assert_eq!(
            expect_cmd(&["ns", "-f", "kube-system"]),
            Command::Ns(NsCommand::Switch {
                target: "kube-system".to_owned(),
                force: true,
            })
        );
    }

    #[test]
    fn ns_subcommand_does_not_modify_context() {
        let cmd = Command::Ns(NsCommand::Switch {
            target: "default".to_owned(),
            force: false,
        });
        assert!(!cmd.modifies_context());
    }

    // -- Completion --

    #[test]
    fn completion_bash_parses() {
        assert_eq!(
            expect_cmd(&["--completion", "bash"]),
            Command::Completion {
                shell: crate::completion::Shell::Bash
            }
        );
    }

    #[test]
    fn completion_without_shell_is_error() {
        let err = expect_err(&["--completion"]);
        assert!(err.contains("requires a shell name"), "{err}");
    }

    #[test]
    fn completion_with_extra_args_is_error() {
        let err = expect_err(&["--completion", "bash", "extra"]);
        assert!(err.contains("takes exactly one argument"), "{err}");
    }

    // ===== kubens parse_ns_args tests =====

    fn parse_ns(input: &[&str]) -> anyhow::Result<ParseResult<NsCommand>> {
        parse_ns_args(&args(input))
    }

    fn expect_ns_cmd(input: &[&str]) -> NsCommand {
        match parse_ns(input).expect("should parse successfully") {
            ParseResult::Run(cmd) => cmd,
            ParseResult::Exit => panic!("expected an NsCommand, got Exit"),
        }
    }

    fn expect_ns_exit(input: &[&str]) {
        match parse_ns(input).expect("should parse successfully") {
            ParseResult::Exit => {}
            ParseResult::Run(cmd) => panic!("expected Exit, got {cmd:?}"),
        }
    }

    fn expect_ns_err(input: &[&str]) -> String {
        parse_ns(input)
            .expect_err("should fail to parse")
            .to_string()
    }

    // -- No args -> List --

    #[test]
    fn ns_no_args_produces_list() {
        assert_eq!(expect_ns_cmd(&[]), NsCommand::List);
    }

    // -- Current namespace --

    #[test]
    fn ns_flag_c_produces_current() {
        assert_eq!(expect_ns_cmd(&["-c"]), NsCommand::Current);
    }

    #[test]
    fn ns_flag_current_produces_current() {
        assert_eq!(expect_ns_cmd(&["--current"]), NsCommand::Current);
    }

    #[test]
    fn ns_current_rejects_extra_args() {
        let err = expect_ns_err(&["-c", "foo"]);
        assert!(
            err.contains("does not accept additional arguments"),
            "{err}"
        );
    }

    // -- Unset --

    #[test]
    fn ns_flag_u_produces_unset() {
        assert_eq!(expect_ns_cmd(&["-u"]), NsCommand::Unset);
    }

    #[test]
    fn ns_flag_unset_produces_unset() {
        assert_eq!(expect_ns_cmd(&["--unset"]), NsCommand::Unset);
    }

    #[test]
    fn ns_unset_rejects_extra_args() {
        let err = expect_ns_err(&["-u", "foo"]);
        assert!(
            err.contains("does not accept additional arguments"),
            "{err}"
        );
    }

    // -- Swap previous --

    #[test]
    fn ns_dash_produces_swap_previous() {
        assert_eq!(expect_ns_cmd(&["-"]), NsCommand::SwapPrevious);
    }

    #[test]
    fn ns_dash_rejects_extra_args() {
        let err = expect_ns_err(&["-", "foo"]);
        assert!(
            err.contains("does not accept additional arguments"),
            "{err}"
        );
    }

    // -- Switch --

    #[test]
    fn ns_bare_name_produces_switch() {
        assert_eq!(
            expect_ns_cmd(&["kube-system"]),
            NsCommand::Switch {
                target: "kube-system".to_owned(),
                force: false,
            }
        );
    }

    // -- Force switch --

    #[test]
    fn ns_flag_f_with_name_produces_forced_switch() {
        assert_eq!(
            expect_ns_cmd(&["-f", "kube-system"]),
            NsCommand::Switch {
                target: "kube-system".to_owned(),
                force: true,
            }
        );
    }

    #[test]
    fn ns_flag_force_with_name_produces_forced_switch() {
        assert_eq!(
            expect_ns_cmd(&["--force", "kube-system"]),
            NsCommand::Switch {
                target: "kube-system".to_owned(),
                force: true,
            }
        );
    }

    #[test]
    fn ns_name_then_f_produces_forced_switch() {
        assert_eq!(
            expect_ns_cmd(&["kube-system", "-f"]),
            NsCommand::Switch {
                target: "kube-system".to_owned(),
                force: true,
            }
        );
    }

    #[test]
    fn ns_name_then_force_produces_forced_switch() {
        assert_eq!(
            expect_ns_cmd(&["kube-system", "--force"]),
            NsCommand::Switch {
                target: "kube-system".to_owned(),
                force: true,
            }
        );
    }

    #[test]
    fn ns_force_without_name_is_error() {
        let err = expect_ns_err(&["-f"]);
        assert!(err.contains("requires a namespace name"), "{err}");
    }

    #[test]
    fn ns_force_long_without_name_is_error() {
        let err = expect_ns_err(&["--force"]);
        assert!(err.contains("requires a namespace name"), "{err}");
    }

    #[test]
    fn ns_force_with_extra_args_is_error() {
        let err = expect_ns_err(&["-f", "ns", "extra"]);
        assert!(err.contains("takes exactly one argument"), "{err}");
    }

    // -- Fzf flag --

    #[test]
    fn ns_fzf_flag_produces_interactive_fzf() {
        assert_eq!(expect_ns_cmd(&["--fzf"]), NsCommand::InteractiveFzf);
    }

    #[test]
    fn ns_fzf_rejects_extra_args() {
        let err = expect_ns_err(&["--fzf", "foo"]);
        assert!(
            err.contains("does not accept additional arguments"),
            "{err}"
        );
    }

    // -- Raw flag --

    #[test]
    fn ns_raw_flag_produces_list_raw() {
        assert_eq!(expect_ns_cmd(&["--raw"]), NsCommand::ListRaw);
    }

    #[test]
    fn ns_raw_rejects_extra_args() {
        let err = expect_ns_err(&["--raw", "foo"]);
        assert!(
            err.contains("does not accept additional arguments"),
            "{err}"
        );
    }

    // -- Help and version --

    #[test]
    fn ns_help_short() {
        expect_ns_exit(&["-h"]);
    }

    #[test]
    fn ns_help_long() {
        expect_ns_exit(&["--help"]);
    }

    #[test]
    fn ns_version_short() {
        expect_ns_exit(&["-V"]);
    }

    #[test]
    fn ns_version_long() {
        expect_ns_exit(&["--version"]);
    }

    // -- Unknown flags --

    #[test]
    fn ns_unknown_flag_is_error() {
        let err = expect_ns_err(&["--foobar"]);
        assert!(err.contains("unknown flag"), "{err}");
    }

    #[test]
    fn ns_unknown_short_flag_is_error() {
        let err = expect_ns_err(&["-x"]);
        assert!(err.contains("unknown flag"), "{err}");
    }

    // -- Extra args after name --

    #[test]
    fn ns_name_with_unexpected_extra_is_error() {
        let err = expect_ns_err(&["kube-system", "extra"]);
        assert!(err.contains("unexpected extra argument"), "{err}");
    }

    #[test]
    fn ns_name_with_multiple_extra_is_error() {
        let err = expect_ns_err(&["kube-system", "-f", "extra"]);
        assert!(err.contains("unexpected extra arguments"), "{err}");
    }

    // -- kubens Completion --

    #[test]
    fn ns_completion_bash_parses() {
        assert_eq!(
            expect_ns_cmd(&["--completion", "bash"]),
            NsCommand::Completion {
                shell: crate::completion::Shell::Bash
            }
        );
    }

    #[test]
    fn ns_completion_zsh_parses() {
        assert_eq!(
            expect_ns_cmd(&["--completion", "zsh"]),
            NsCommand::Completion {
                shell: crate::completion::Shell::Zsh
            }
        );
    }

    #[test]
    fn ns_completion_fish_parses() {
        assert_eq!(
            expect_ns_cmd(&["--completion", "fish"]),
            NsCommand::Completion {
                shell: crate::completion::Shell::Fish
            }
        );
    }

    #[test]
    fn ns_completion_without_shell_is_error() {
        let err = expect_ns_err(&["--completion"]);
        assert!(err.contains("requires a shell name"), "{err}");
    }

    #[test]
    fn ns_completion_with_extra_args_is_error() {
        let err = expect_ns_err(&["--completion", "bash", "extra"]);
        assert!(err.contains("takes exactly one argument"), "{err}");
    }
}
