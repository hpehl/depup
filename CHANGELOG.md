# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-06-16

### Added

- `update` subcommand for updating outdated dependencies
  - Maven: format-preserving POM updates for both managed properties (`<properties>`) and inline versions (`<version>x.y.z</version>`) — preserves comments, whitespace, and indentation
  - npm: delegates to native package manager update commands (`npm update`, `pnpm update`, `yarn upgrade`, `bun update`)
  - `--dry-run` flag to preview updates without making changes (JSON status: `would_update`)
  - Structured JSON output with `ecosystem`, `kind`, `managed`, `artifact`, `source`, `old_version`, `new_version` fields
  - Summary line, elapsed time, progress bar, and exit code 1 on errors (mirrors `check` output)
- `--include`/`--exclude` glob filters for `check`, `update`, and `audit` (e.g., `--include 'org.junit:*'`, `--exclude '*:guava'`, `--include 'react*'`)
- `audit` subcommand for checking dependencies against known vulnerabilities via [OSV.dev](https://osv.dev/)
  - Queries both Maven and npm ecosystems using the OSV batch API
  - Fetches full vulnerability details including CVE aliases, severity (CVSS-based), summaries, and advisory URLs
  - `--severity` filter to show only vulnerabilities at or above a threshold (critical, high, medium, low)
  - Supports all check filters: `--maven`/`--npm`, `--dependencies`/`--plugins`/`--dev-deps`, `--managed`/`--unmanaged`, `--include`/`--exclude`
  - Structured JSON output with vulnerability details
  - Grouped table output with severity-colored labels, summary line, and timing
  - Exit code 1 when vulnerabilities are found
  - Tool versions (Node.js, package managers) are skipped

### Changed

- Rename crate to `depup-cli` for crates.io publishing (`cargo install depup-cli` installs the `depup` binary)
- Audit `--severity` filter now drops dependencies whose vulnerabilities were all below the threshold instead of showing them as clean

## [0.1.0] - 2026-06-15

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

[Unreleased]: https://github.com/hpehl/depup/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/hpehl/depup/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/hpehl/depup/releases/tag/v0.1.0
