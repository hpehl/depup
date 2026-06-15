# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `check` subcommand with auto-detection of ecosystems (Maven + npm)
- `completions` subcommand to generate and install shell completions (bash, zsh, fish, elvish, powershell)
- Maven ecosystem: multi-module project discovery, `${version.*}` property resolution, plain version number checking, custom repository support (`<repositories>` and `<pluginRepositories>`)
- Maven ecosystem: Node.js and package manager version properties (`version.node`, `version.npm`, `version.pnpm`, `version.yarn`)
- npm ecosystem: auto-detect package manager by lock file or `packageManager` field in `package.json`
- npm ecosystem: support for npm, pnpm, yarn (classic), and bun
- npm ecosystem: workspace-aware discovery (skips workspace members)
- Table and JSON output formats with results grouped by ecosystem and kind
- `--outdated` flag to show only outdated dependencies
- `--stable` / `--releases-only` flag to exclude pre-release versions (alpha, beta, RC, milestone)
- Progress bars during version checks (hidden in JSON mode)
- Structured error types with machine-parseable error codes and JSON error envelope
- Exit code 1 when outdated dependencies are found

[Unreleased]: https://github.com/hpehl/depup/compare/v0.1.0...HEAD
