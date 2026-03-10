# kubehop

Fast Kubernetes context and namespace switcher, written in Rust.
Drop-in replacement for
[kubectx/kubens](https://github.com/ahmetb/kubectx).

<!-- TODO: record and replace with actual GIFs -->

### Context switching (`kubectx`)

![kubectx demo](docs/assets/kubectx-demo.gif)

### Namespace switching (`kubens`)

![kubens demo](docs/assets/kubens-demo.gif)

## Why

The Go version of kubectx parses the full kubeconfig YAML three
times during an interactive switch and spawns two subprocesses
(fzf + a re-invocation of itself). kubehop eliminates all of
that:

- **Single parse** of the kubeconfig, held in memory for the
  entire operation
- **Zero subprocesses** in default mode (built-in fuzzy picker)
- **Selective deserialization** -- only `current-context` and
  context names are parsed; clusters, users, and certificates
  are skipped entirely
- **Sub-50ms** time-to-interactive on typical kubeconfigs

## Install

### From source

```sh
cargo install --git https://github.com/ikchifo/kubehop
```

Then create the symlinks:

```sh
ln -s ~/.cargo/bin/khop ~/.cargo/bin/kubectx
ln -s ~/.cargo/bin/khop ~/.cargo/bin/kubens
```

### Binary name

The crate builds a single binary called `khop`. Behavior is
determined by argv0:

| Invoked as | Mode |
|---|---|
| `kubectx`, `kubectl-ctx`, `khop` | Context switching |
| `kubens`, `kubectl-ns` | Namespace switching |

## Usage

### kubectx

```
kubectx                   List contexts (interactive if TTY)
kubectx <name>            Switch to context
kubectx -                 Switch to previous context
kubectx <new>=<old>       Rename context
kubectx -c, --current     Show current context name
kubectx -d, --delete <n>  Delete context
kubectx -u, --unset       Unset current context
kubectx --fzf             Use external fzf for selection
kubectx --completion <sh> Output shell completion (bash/zsh/fish)
```

### kubens

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
kubectx --completion bash > ~/.local/share/bash-completion/completions/kubectx

# zsh
kubectx --completion zsh > ~/.zfunc/_kubectx

# fish
kubectx --completion fish > ~/.config/fish/completions/kubectx.fish
```

Pre-generated scripts are also available in the `completion/`
directory.

## k9s plugin

kubehop can be used as a
[k9s plugin](https://k9scli.io/topics/plugins/) for context
switching. Copy `examples/plugins.yaml` into your k9s config
directory, or add the following to your existing plugins file:

```yaml
plugins:
  kubehop:
    shortCut: Ctrl-K
    description: Switch context
    scopes:
      - all
    command: kubectx
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
  round-trip for writes (preserves all fields)
- **Multi-file `KUBECONFIG`** support with first-occurrence-wins
  dedup
- **No async runtime** -- all operations are local file I/O on
  small files
- **`thiserror`** in library modules, **`anyhow`** in the CLI
  layer

## License

Apache-2.0
