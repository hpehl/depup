# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `check` subcommand (default) to check Maven version properties against upstream registries
- `completions` subcommand to generate and install shell completions (bash, zsh, fish, elvish, powershell)
- Progress bars with spinners during version checks
- Structured error types with machine-parseable error codes in JSON mode
- JSON error envelope for scripting support
- Custom repository support from POM-defined `<repositories>` and `<pluginRepositories>`
- Pre-release version filtering (alpha, beta, RC, milestone, SNAPSHOT)
- Multi-module Maven project discovery
- Table and JSON output formats

[Unreleased]: https://github.com/hpehl/depup/compare/v0.1.0...HEAD
