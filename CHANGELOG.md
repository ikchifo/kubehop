# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.2](https://github.com/ikchifo/kubehop/compare/v0.2.1...v0.2.2) - 2026-03-11

### Added

- add khop ns subcommand and kubens shell completions ([#3](https://github.com/ikchifo/kubehop/pull/3))

## [0.2.1](https://github.com/ikchifo/kubehop/compare/v0.2.0...v0.2.1) - 2026-03-11

### Added

- add picker navigation, colored list output, and --raw flag

### Fixed

- improve terminal resize handling in picker
- handle Ctrl+C and Ctrl+Z in the interactive picker

## [0.2.0](https://github.com/ikchifo/kubehop/compare/v0.1.0...v0.2.0) - 2026-03-11

### Added

- add release-plz for automated versioning and changelog
- add Criterion benchmark suite and CI bench compile check
- improve error reporting, safety, and CLI ergonomics

### Other

- simplify README install section
- update README with install methods, CLI changes, and prose tightening
- bump to edition 2024, remove compliance comments
