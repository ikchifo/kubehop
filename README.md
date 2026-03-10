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

- **Built-in fuzzy picker** powered by nucleo (from the Helix
  editor) and ratatui -- no fzf required
- **Single kubeconfig parse** per operation, held in memory for
  the entire flow
- **Selective deserialization** -- only `current-context` and
  context names are parsed; clusters, users, and certificates
  are never allocated
- **Multi-file `KUBECONFIG`** support with first-occurrence-wins
  dedup
- **Sub-50ms** time-to-interactive on typical kubeconfigs
- **k9s plugin** support via a `pick` subcommand
- **Shell completions** for bash, zsh, and fish
- Optional **fzf fallback** via `--fzf` flag

## Install

### From source

```sh
cargo install --git https://github.com/ikchifo/kubehop
```

The binary is called `khop`. You can use it directly:

```sh
khop              # context switching mode (default)
khop -c           # show current context
```

### Optional: kubectx/kubens symlinks

If you want the familiar `kubectx`/`kubens` command names, or
compatibility with tools that expect them, create symlinks:

```sh
ln -s ~/.cargo/bin/khop ~/.cargo/bin/kubectx
ln -s ~/.cargo/bin/khop ~/.cargo/bin/kubens
```

Behavior is determined by argv0:

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

When invoked as `kubens` (via symlink) or with the namespace
mode binary name:

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

Pre-generated scripts are also available in the `completion/`
directory.

## k9s plugin

kubehop can be used as a
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
  main.rs           Thin entry point (~10 lines)
  lib.rs            Crate root, module wiring
  cli.rs            Arg parsing and command dispatch
  dispatch.rs       Argv0-based mode detection
  completion.rs     Shell completion generation
  kubeconfig/       Selective serde parser, multi-file merge
  context/          List, switch, rename, delete, unset, state
  namespace/        List, switch, unset, per-context state
  picker/           nucleo fuzzy scoring + ratatui inline TUI
  integration/      k9s plugin subcommand
```

Key design decisions:

- **Selective serde** for reads, full `serde_yaml::Value`
  round-trip for writes (preserves all fields through
  mutations)
- **No async runtime** -- all operations are local file I/O
  on small files
- **`thiserror`** in library modules, **`anyhow`** in the
  CLI layer
- **State file compatibility** -- previous-context state is
  stored at the same path kubectx uses, so `khop -` works
  if you are migrating

## License

Apache-2.0
