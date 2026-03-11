# kubehop

[![CI](https://github.com/ikchifo/kubehop/actions/workflows/ci.yml/badge.svg)](https://github.com/ikchifo/kubehop/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/khop.svg)](https://crates.io/crates/khop)
[![License](https://img.shields.io/crates/l/khop.svg)](LICENSE)

Fast Kubernetes context and namespace switcher. Drop-in
replacement for
[kubectx/kubens](https://github.com/ahmetb/kubectx) with a
built-in fuzzy finder, sub-50ms startup, and no runtime
dependencies. Written in Rust.

## Features

- **Built-in fuzzy picker** powered by
  [nucleo](https://github.com/helix-editor/nucleo) (from the
  [Helix](https://helix-editor.com/) editor) and
  [ratatui](https://github.com/ratatui/ratatui) -- no fzf
  required
- **Single kubeconfig parse** per invocation -- parsed once,
  used for the entire operation
- **Selective parsing** -- reads only `current-context` and
  context names; skips clusters, users, and certificates
- **Multi-file `KUBECONFIG`** support -- when names overlap,
  the first file wins
- **Sub-50ms startup** on standard kubeconfig files
- **Batch operations** -- delete multiple contexts in one
  command
- **k9s plugin** support via a `pick` subcommand
- **Shell completions** for bash, zsh, and fish with
  dynamic context/namespace name completion
- **`--raw` output** for scripting (no prefixes, no color)
- **Recency-aware picker** -- recently switched contexts and
  namespaces appear first
- Optional **[fzf](https://github.com/junegunn/fzf)
  fallback** via `--fzf` flag

## Install

### Homebrew

```sh
brew install ikchifo/tap/khop
```

### Shell installer (macOS / Linux)

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/ikchifo/kubehop/releases/latest/download/khop-installer.sh | sh
```

### Cargo

```sh
cargo install khop
```

Pre-built binaries and a PowerShell installer are also
available on the
[releases page](https://github.com/ikchifo/kubehop/releases/latest).

### Optional: kubectx/kubens symlinks

To use the familiar `kubectx`/`kubens` command names, create
symlinks pointing to `khop`. For example, with a Cargo
install:

```sh
ln -s ~/.cargo/bin/khop ~/.cargo/bin/kubectx
ln -s ~/.cargo/bin/khop ~/.cargo/bin/kubens
```

The binary name (argv0) determines the default mode:

| Invoked as | Mode |
|---|---|
| `khop`, `kubectx`, `kubectl-ctx` | Context switching |
| `khop ns`, `kubens`, `kubectl-ns` | Namespace switching |

## Usage

### Context switching

```
khop                             List contexts (interactive if TTY)
khop <name>                      Switch to context
khop -                           Switch to previous context
khop <new>=<old>                 Rename context ('.' for current)
khop -c, --current               Show current context name
khop -d, --delete NAME [NAME...] Delete context(s) ('.' for current)
khop -u, --unset                 Unset current context
khop --raw                       List context names (no prefix, no color)
khop --fzf                       Use external fzf for selection
khop ns <args>                   Namespace mode (see below)
khop --completion <shell>        Output shell completion (bash/zsh/fish)
```

### Namespace switching

Use `khop ns` or invoke the binary as `kubens` / `kubectl-ns`
(see [symlinks](#optional-kubectxkubens-symlinks) above):

```
khop ns                     List namespaces (interactive if TTY)
khop ns <name>              Switch namespace
khop ns -                   Switch to previous namespace
khop ns -f, --force <name>  Switch without existence check
khop ns <name> -f           Same, trailing form
khop ns -c, --current       Show current namespace
khop ns -u, --unset         Reset namespace to "default"
khop ns --raw               List namespace names (no prefix, no color)
khop ns --fzf               Use external fzf for selection
khop ns --completion <shell> Output shell completion (bash/zsh/fish)
```

All `khop ns` commands also work when invoked as `kubens`
(e.g. `kubens kube-system`).

## Shell completions

Context and namespace names are completed dynamically.

```sh
# bash (kubectx)
khop --completion bash > ~/.local/share/bash-completion/completions/khop

# zsh (kubectx)
khop --completion zsh > ~/.zfunc/_khop

# fish (kubectx)
khop --completion fish > ~/.config/fish/completions/khop.fish

# kubens (if using symlinks)
kubens --completion bash > ~/.local/share/bash-completion/completions/kubens
kubens --completion zsh  > ~/.zfunc/_kubens
kubens --completion fish > ~/.config/fish/completions/kubens.fish
```

## k9s plugin

kubehop works as a
[k9s plugin](https://k9scli.io/topics/plugins/) for context
switching. Add this to your k9s `plugins.yaml`:

```yaml
plugins:
  kubehop:
    shortCut: Ctrl-K
    description: Switch context
    scopes:
      - all
    command: khop
    args:
      - pick
      - --switch
```

## Architecture

```
src/
  main.rs           Entry point (~10 lines)
  lib.rs            Crate root, module declarations
  cli.rs            Arg parsing and command dispatch
  dispatch.rs       Argv0-based mode detection
  completion.rs     Shell completion generation
  kubeconfig/       Selective serde parser, multi-file merge
  context/          Current, switch, mutate (rename/delete), state
  namespace/        List, switch, unset, per-context state
  picker/           Fuzzy scoring (nucleo) + inline TUI (ratatui)
  integration/      k9s plugin subcommand
```

Design decisions:

- **Selective [serde](https://serde.rs/)** for reads, full
  [`serde_yaml::Value`](https://docs.rs/serde_yaml) round-trip
  for writes, preserving fields the tool does not use
- **No async runtime** -- all operations are local file I/O
  on small files
- **[`thiserror`](https://docs.rs/thiserror)** in library
  modules,
  **[`anyhow`](https://docs.rs/anyhow)** in the CLI layer
- **State file compatibility** -- shares its state file path
  with [kubectx](https://github.com/ahmetb/kubectx), so
  `khop -` works after migrating

## License

Apache-2.0
