# kubehop

Fast Kubernetes context and namespace switcher, written in
Rust. Built-in fuzzy finder, sub-50ms startup, no external
dependencies at runtime.

<!-- TODO: record and replace with actual GIFs -->

### Context switching

![kubectx demo](docs/assets/kubectx-demo.gif)

### Namespace switching

![kubens demo](docs/assets/kubens-demo.gif)

## Features

- **Built-in fuzzy picker** powered by
  [nucleo](https://github.com/helix-editor/nucleo) (from the
  [Helix](https://helix-editor.com/) editor) and
  [ratatui](https://github.com/ratatui/ratatui) -- no fzf
  required
- **Single kubeconfig parse** per operation, held in memory
  for the entire flow
- **Selective parsing** -- reads only `current-context` and
  context names; skips clusters, users, and certificates
  entirely
- **Multi-file `KUBECONFIG`** support -- when names overlap,
  the first file wins
- **Sub-50ms startup** on standard kubeconfig files
- **k9s plugin** support via a `pick` subcommand
- **Shell completions** for bash, zsh, and fish
- Optional **[fzf](https://github.com/junegunn/fzf)
  fallback** via `--fzf` flag

## Install

### From source

```sh
cargo install --git https://github.com/ikchifo/kubehop
```

This installs a binary called `khop`:

```sh
khop              # context switching mode (default)
khop -c           # show current context
```

### Optional: kubectx/kubens symlinks

To use the `kubectx`/`kubens` command names, or for
compatibility with tools that expect them, create symlinks:

```sh
ln -s ~/.cargo/bin/khop ~/.cargo/bin/kubectx
ln -s ~/.cargo/bin/khop ~/.cargo/bin/kubens
```

The tool detects its mode from the command name you use:

| Invoked as | Mode |
|---|---|
| `khop`, `kubectx`, `kubectl-ctx` | Context switching |
| `kubens`, `kubectl-ns` | Namespace switching |

## Usage

### Context switching

```
khop                      List contexts (interactive if TTY)
khop <name>               Switch to context
khop -                    Switch to previous context
khop <new>=<old>          Rename context
khop -c, --current        Show current context name
khop -d, --delete <name>  Delete context
khop -u, --unset          Unset current context
khop --fzf                Use external fzf for selection
khop --completion <shell> Output shell completion (bash/zsh/fish)
```

### Namespace switching

When invoked as `kubens` or `kubectl-ns` (via symlink):

```
kubens                    List namespaces (interactive if TTY)
kubens <name>             Switch namespace
kubens -                  Switch to previous namespace
kubens <name> -f          Switch without existence check
kubens -c, --current      Show current namespace
kubens -u, --unset        Reset namespace to "default"
kubens --fzf              Use external fzf for selection
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

The `completion/` directory has pre-generated scripts.

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
  context/          List, switch, rename, delete, unset, state
  namespace/        List, switch, unset, per-context state
  picker/           Fuzzy scoring (nucleo) + inline TUI (ratatui)
  integration/      k9s plugin subcommand
```

Design decisions:

- **Selective [serde](https://serde.rs/)** for reads, full
  [`serde_yaml::Value`](https://docs.rs/serde_yaml) round-trip
  for writes (so edits never drop unknown fields)
- **No async runtime** -- all operations are local file I/O
  on small files
- **[`thiserror`](https://docs.rs/thiserror)** in library
  modules,
  **[`anyhow`](https://docs.rs/anyhow)** in the CLI layer
- **State file compatibility** -- stores previous-context
  state at the same path as
  [kubectx](https://github.com/ahmetb/kubectx), so `khop -`
  works for users migrating from kubectx

## License

Apache-2.0
