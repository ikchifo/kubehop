# Contributing to kubehop

Thanks for your interest in contributing to kubehop. This
document covers the basics.

## Getting started

1. Fork the repository and clone your fork.
2. Create a branch for your change (`git checkout -b my-change`).
3. Make your changes, add tests, and verify everything passes.
4. Push your branch and open a pull request.

## Prerequisites

- Rust stable (edition 2021 or later)
- `cargo`, `clippy`, and `rustfmt` (included with rustup)

## Building and testing

```sh
# Build
cargo build

# Run all tests
cargo test

# Lint
cargo clippy --tests -- -D warnings

# Format check
cargo fmt --check
```

All pull requests must pass `cargo test` and
`cargo clippy --tests -- -D warnings` with zero errors.

## Code style

- Run `cargo fmt` before committing.
- Follow existing patterns in the codebase. When in doubt, look
  at adjacent code for conventions.
- Library modules (`kubeconfig/`, `context/`, `namespace/`,
  `picker/`) use `thiserror` for error types. The CLI layer
  (`cli.rs`) uses `anyhow`.
- Prefer small, focused pull requests over large ones.

## Commit messages

Use [Conventional Commits](https://www.conventionalcommits.org/)
format:

```
feat: add support for KUBECONFIG_DEFAULT_CONTEXT
fix: handle empty namespace field in kubeconfig
refactor: extract shared YAML helpers
test: add integration tests for context switching
docs: update README with install instructions
```

Keep the first line under 72 characters. Use the body for
additional context if needed.

## Adding tests

- Unit tests go in the same file as the code they test, inside
  a `#[cfg(test)] mod tests { }` block.
- Integration tests go in `tests/`. Shared helpers live in
  `tests/common/mod.rs`.
- Use descriptive test names that explain the scenario, not just
  the function being tested.

## Reporting issues

Open an issue on GitHub. Include:

- What you expected to happen
- What actually happened
- Your kubeconfig setup (redact sensitive values)
- Output of `kubectx --version` or `kubens --version`

## License

By contributing, you agree that your contributions will be
licensed under the Apache License 2.0.
