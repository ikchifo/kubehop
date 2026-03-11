//! Shell completion script generation for bash, zsh, and fish.

use std::fmt;
use std::str::FromStr;

/// Supported shells for completion generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
}

impl FromStr for Shell {
    type Err = ParseShellError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "bash" => Ok(Self::Bash),
            "zsh" => Ok(Self::Zsh),
            "fish" => Ok(Self::Fish),
            _ => Err(ParseShellError(s.to_owned())),
        }
    }
}

/// Error returned when an unknown shell name is passed to [`Shell::from_str`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseShellError(String);

impl fmt::Display for ParseShellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown shell {:?} (expected bash, zsh, or fish)",
            self.0
        )
    }
}

impl std::error::Error for ParseShellError {}

/// Generate a kubectx completion script for the given shell.
///
/// The returned string is ready to be written to a file or piped into
/// the shell's completion loading mechanism.
#[must_use]
pub fn generate(shell: Shell) -> String {
    match shell {
        Shell::Bash => generate_bash(),
        Shell::Zsh => generate_zsh(),
        Shell::Fish => generate_fish(),
    }
}

/// Generate a kubens completion script for the given shell.
///
/// The returned string is ready to be written to a file or piped into
/// the shell's completion loading mechanism.
#[must_use]
pub fn generate_kubens(shell: Shell) -> String {
    match shell {
        Shell::Bash => generate_kubens_bash(),
        Shell::Zsh => generate_kubens_zsh(),
        Shell::Fish => generate_kubens_fish(),
    }
}

// -- kubectx completions --

fn generate_bash() -> String {
    r#"# bash completion for kubectx
_kubectx() {
    local cur prev
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"

    local flags="-c --current -d --delete -u --unset --fzf --raw -h --help -V --version --completion"

    case "$prev" in
        -d|--delete)
            local contexts
            contexts="$(khop --raw 2>/dev/null || kubectx --raw 2>/dev/null)"
            COMPREPLY=( $(compgen -W "$contexts" -- "$cur") )
            return
            ;;
        --completion)
            COMPREPLY=( $(compgen -W "bash zsh fish" -- "$cur") )
            return
            ;;
    esac

    if [[ "$cur" == -* ]]; then
        COMPREPLY=( $(compgen -W "$flags" -- "$cur") )
        return
    fi

    if [[ "${COMP_WORDS[1]}" == "pick" ]]; then
        local pick_flags="--switch --kubeconfig --current"
        COMPREPLY=( $(compgen -W "$pick_flags" -- "$cur") )
        return
    fi

    local contexts
    contexts="$(khop --raw 2>/dev/null || kubectx --raw 2>/dev/null)"
    COMPREPLY=( $(compgen -W "$contexts pick" -- "$cur") )
}

complete -F _kubectx kubectx
"#
    .to_owned()
}

fn generate_zsh() -> String {
    r#"#compdef kubectx

_kubectx() {
    local -a contexts flags

    flags=(
        '-c:show current context name'
        '--current:show current context name'
        '-d:delete a context'
        '--delete:delete a context'
        '-u:unset the current context'
        '--unset:unset the current context'
        '--fzf:use external fzf for selection'
        '--raw:list context names without prefix or color'
        '-h:show help'
        '--help:show help'
        '-V:show version'
        '--version:show version'
        '--completion:output shell completion script'
    )

    if (( CURRENT == 2 )); then
        _alternative \
            'flags:flags:_describe "flag" flags' \
            'contexts:contexts:_kubectx_contexts' \
            'commands:commands:(pick)'
        return
    fi

    case "${words[2]}" in
        -d|--delete)
            _kubectx_contexts
            ;;
        --completion)
            _values 'shell' bash zsh fish
            ;;
        pick)
            local -a pick_flags
            pick_flags=(
                '--switch:switch context after picking'
                '--kubeconfig:kubeconfig file path'
                '--current:current context name'
            )
            _describe 'pick flag' pick_flags
            ;;
    esac
}

_kubectx_contexts() {
    local -a ctx_list
    ctx_list=("${(@f)$(khop --raw 2>/dev/null || kubectx --raw 2>/dev/null)}")
    _describe 'context' ctx_list
}

_kubectx "$@"
"#
    .to_owned()
}

fn generate_fish() -> String {
    r"# fish completion for kubectx
complete -c kubectx -f

# Dynamic context names
complete -c kubectx -f -n '__fish_use_subcommand' -a '(khop --raw 2>/dev/null; or kubectx --raw 2>/dev/null)'

# Flags
complete -c kubectx -f -s c -l current -d 'Show current context name'
complete -c kubectx -f -s d -l delete -d 'Delete a context' -r -a '(khop --raw 2>/dev/null; or kubectx --raw 2>/dev/null)'
complete -c kubectx -f -s u -l unset -d 'Unset the current context'
complete -c kubectx -f -l fzf -d 'Use external fzf for selection'
complete -c kubectx -f -l raw -d 'List context names without prefix or color'
complete -c kubectx -f -s h -l help -d 'Show help'
complete -c kubectx -f -s V -l version -d 'Show version'
complete -c kubectx -f -l completion -d 'Output shell completion script' -r -a 'bash zsh fish'

# pick subcommand
complete -c kubectx -f -n '__fish_use_subcommand' -a pick -d 'Interactive picker (k9s plugin)'
complete -c kubectx -f -n '__fish_seen_subcommand_from pick' -l switch -d 'Switch context after picking'
complete -c kubectx -f -n '__fish_seen_subcommand_from pick' -l kubeconfig -d 'Kubeconfig file path' -r
complete -c kubectx -f -n '__fish_seen_subcommand_from pick' -l current -d 'Current context name' -r
"
    .to_owned()
}

// -- kubens completions --

fn generate_kubens_bash() -> String {
    r#"# bash completion for kubens
_kubens() {
    local cur prev
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"

    local flags="-c --current -u --unset -f --force --fzf --raw -h --help -V --version --completion"

    case "$prev" in
        -f|--force)
            local namespaces
            namespaces="$(kubens --raw 2>/dev/null)"
            COMPREPLY=( $(compgen -W "$namespaces" -- "$cur") )
            return
            ;;
        --completion)
            COMPREPLY=( $(compgen -W "bash zsh fish" -- "$cur") )
            return
            ;;
    esac

    if [[ "$cur" == -* ]]; then
        COMPREPLY=( $(compgen -W "$flags" -- "$cur") )
        return
    fi

    local namespaces
    namespaces="$(kubens --raw 2>/dev/null)"
    COMPREPLY=( $(compgen -W "$namespaces" -- "$cur") )
}

complete -F _kubens kubens
"#
    .to_owned()
}

fn generate_kubens_zsh() -> String {
    r#"#compdef kubens

_kubens() {
    local -a namespaces flags

    flags=(
        '-c:show current namespace'
        '--current:show current namespace'
        '-u:reset namespace to default'
        '--unset:reset namespace to default'
        '-f:switch without checking namespace exists'
        '--force:switch without checking namespace exists'
        '--fzf:use external fzf for selection'
        '--raw:list namespace names without prefix or color'
        '-h:show help'
        '--help:show help'
        '-V:show version'
        '--version:show version'
        '--completion:output shell completion script'
    )

    if (( CURRENT == 2 )); then
        _alternative \
            'flags:flags:_describe "flag" flags' \
            'namespaces:namespaces:_kubens_namespaces'
        return
    fi

    case "${words[2]}" in
        -f|--force)
            _kubens_namespaces
            ;;
        --completion)
            _values 'shell' bash zsh fish
            ;;
    esac
}

_kubens_namespaces() {
    local -a ns_list
    ns_list=("${(@f)$(kubens --raw 2>/dev/null)}")
    _describe 'namespace' ns_list
}

_kubens "$@"
"#
    .to_owned()
}

fn generate_kubens_fish() -> String {
    r"# fish completion for kubens
complete -c kubens -f

# Dynamic namespace names
complete -c kubens -f -n '__fish_use_subcommand' -a '(kubens --raw 2>/dev/null)'

# Flags
complete -c kubens -f -s c -l current -d 'Show current namespace'
complete -c kubens -f -s u -l unset -d 'Reset namespace to default'
complete -c kubens -f -s f -l force -d 'Switch without checking namespace exists' -r -a '(kubens --raw 2>/dev/null)'
complete -c kubens -f -l fzf -d 'Use external fzf for selection'
complete -c kubens -f -l raw -d 'List namespace names without prefix or color'
complete -c kubens -f -s h -l help -d 'Show help'
complete -c kubens -f -s V -l version -d 'Show version'
complete -c kubens -f -l completion -d 'Output shell completion script' -r -a 'bash zsh fish'
"
    .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bash() {
        assert_eq!("bash".parse::<Shell>().unwrap(), Shell::Bash);
    }

    #[test]
    fn parse_zsh() {
        assert_eq!("zsh".parse::<Shell>().unwrap(), Shell::Zsh);
    }

    #[test]
    fn parse_fish() {
        assert_eq!("fish".parse::<Shell>().unwrap(), Shell::Fish);
    }

    #[test]
    fn parse_case_insensitive() {
        assert_eq!("BASH".parse::<Shell>().unwrap(), Shell::Bash);
        assert_eq!("Zsh".parse::<Shell>().unwrap(), Shell::Zsh);
        assert_eq!("FISH".parse::<Shell>().unwrap(), Shell::Fish);
    }

    #[test]
    fn parse_unknown_shell_errors() {
        let err = "powershell".parse::<Shell>().unwrap_err();
        assert!(err.to_string().contains("unknown shell"));
        assert!(err.to_string().contains("powershell"));
    }

    // -- kubectx completion tests --

    #[test]
    fn bash_output_has_compreply() {
        let output = generate(Shell::Bash);
        assert!(
            output.contains("COMPREPLY"),
            "bash script must use COMPREPLY"
        );
        assert!(output.contains("compgen"), "bash script must use compgen");
        assert!(
            output.contains("complete -F _kubectx kubectx"),
            "bash script must register the completion function"
        );
    }

    #[test]
    fn zsh_output_has_compdef() {
        let output = generate(Shell::Zsh);
        assert!(
            output.contains("#compdef kubectx"),
            "zsh script must start with #compdef"
        );
        assert!(
            output.contains("_kubectx"),
            "zsh script must define _kubectx"
        );
    }

    #[test]
    fn fish_output_has_complete_command() {
        let output = generate(Shell::Fish);
        assert!(
            output.contains("complete -c kubectx"),
            "fish script must use complete -c"
        );
    }

    #[test]
    fn all_shells_use_khop_raw_for_contexts() {
        for shell in [Shell::Bash, Shell::Zsh, Shell::Fish] {
            let output = generate(shell);
            assert!(
                output.contains("khop --raw"),
                "{shell:?} script must fetch contexts via khop --raw"
            );
        }
    }

    #[test]
    fn all_shells_have_kubectx_raw_fallback() {
        for shell in [Shell::Bash, Shell::Zsh, Shell::Fish] {
            let output = generate(shell);
            assert!(
                output.contains("kubectx --raw"),
                "{shell:?} script must fall back to kubectx --raw"
            );
        }
    }

    #[test]
    fn all_shells_include_raw_flag() {
        for shell in [Shell::Bash, Shell::Zsh, Shell::Fish] {
            let output = generate(shell);
            assert!(
                output.contains("--raw"),
                "{shell:?} script must list --raw in flags"
            );
        }
    }

    // -- kubens completion tests --

    #[test]
    fn kubens_bash_output_has_compreply() {
        let output = generate_kubens(Shell::Bash);
        assert!(
            output.contains("COMPREPLY"),
            "kubens bash script must use COMPREPLY"
        );
        assert!(
            output.contains("compgen"),
            "kubens bash script must use compgen"
        );
        assert!(
            output.contains("complete -F _kubens kubens"),
            "kubens bash script must register the completion function"
        );
    }

    #[test]
    fn kubens_zsh_output_has_compdef() {
        let output = generate_kubens(Shell::Zsh);
        assert!(
            output.contains("#compdef kubens"),
            "kubens zsh script must start with #compdef"
        );
        assert!(
            output.contains("_kubens"),
            "kubens zsh script must define _kubens"
        );
    }

    #[test]
    fn kubens_fish_output_has_complete_command() {
        let output = generate_kubens(Shell::Fish);
        assert!(
            output.contains("complete -c kubens"),
            "kubens fish script must use complete -c"
        );
    }

    #[test]
    fn kubens_all_shells_reference_namespace_listing() {
        for shell in [Shell::Bash, Shell::Zsh, Shell::Fish] {
            let output = generate_kubens(shell);
            assert!(
                output.contains("kubens --raw"),
                "{shell:?} kubens script must fetch namespaces via kubens --raw"
            );
        }
    }

    #[test]
    fn kubens_all_shells_include_raw_flag() {
        for shell in [Shell::Bash, Shell::Zsh, Shell::Fish] {
            let output = generate_kubens(shell);
            assert!(
                output.contains("--raw"),
                "{shell:?} kubens script must list --raw in flags"
            );
        }
    }

    #[test]
    fn kubens_all_shells_include_force_flag() {
        for shell in [Shell::Bash, Shell::Zsh, Shell::Fish] {
            let output = generate_kubens(shell);
            let has_force = output.contains("--force") || output.contains("-l force");
            assert!(has_force, "{shell:?} kubens script must list force flag");
        }
    }

    #[test]
    fn kubens_all_shells_include_completion_flag() {
        for shell in [Shell::Bash, Shell::Zsh, Shell::Fish] {
            let output = generate_kubens(shell);
            let has_completion =
                output.contains("--completion") || output.contains("-l completion");
            assert!(
                has_completion,
                "{shell:?} kubens script must list completion flag"
            );
        }
    }
}
