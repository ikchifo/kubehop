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
- **Shell completions** for bash, zsh, and fish
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

### PowerShell installer (Windows)

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/ikchifo/kubehop/releases/latest/download/khop-installer.ps1 | iex"
```

### Pre-built binaries

Download from the
[latest release](https://github.com/ikchifo/kubehop/releases/latest).
Each release includes binaries for:

- `x86_64-apple-darwin` (macOS Intel)
- `aarch64-apple-darwin` (macOS Apple Silicon)
- `x86_64-unknown-linux-gnu` (Linux x64)
- `aarch64-unknown-linux-gnu` (Linux ARM64)
- `x86_64-pc-windows-msvc` (Windows x64)

### From crates.io

```sh
cargo install khop
```

### From source

```sh
cargo install --git https://github.com/ikchifo/kubehop
```

After installing, you have a binary called `khop`:

```sh
khop              # context switching mode (default)
khop -c           # show current context
```

### Optional: kubectx/kubens symlinks

To use the familiar `kubectx`/`kubens` command names, create
symlinks pointing to `khop`. For example, with a Cargo
install:

```sh
ln -s ~/.cargo/bin/khop ~/.cargo/bin/kubectx
ln -s ~/.cargo/bin/khop ~/.cargo/bin/kubens
```

The binary name (argv0) determines the mode:

| Invoked as | Mode |
|---|---|
| `khop`, `kubectx`, `kubectl-ctx` | Context switching |
| `kubens`, `kubectl-ns` | Namespace switching |

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
khop --fzf                       Use external fzf for selection
khop --completion <shell>        Output shell completion (bash/zsh/fish)
```

### Namespace switching

Namespace switching requires invoking the binary as `kubens`
or `kubectl-ns` (see
[symlinks](#optional-kubectxkubens-symlinks) above):

```
kubens                      List namespaces (interactive if TTY)
kubens <name>               Switch namespace
kubens -                    Switch to previous namespace
kubens -f, --force <name>   Switch without existence check
kubens <name> -f            Same, trailing form
kubens -c, --current        Show current namespace
kubens -u, --unset          Reset namespace to "default"
kubens --fzf                Use external fzf for selection
```

## Shell completions

```sh
# bash
khop --completion bash > ~/.local/share/bash-completion/completions/khop

# zsh
khop --completion zsh > ~/.zfunc/_khop

# fish
khop --completion fish > ~/.config/fish/completions/khop.fish
```

The `completion/` directory has pre-generated scripts for
both `kubectx` and `kubens`.

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
