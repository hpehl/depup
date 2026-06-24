# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- Fix npm update silently skipping cross-major version bumps — replace blanket `<pm> update` commands with targeted `<pm> add <pkg>@<version>` installs that rewrite `package.json` ranges correctly for all four package managers (npm, pnpm, yarn, bun)

## [1.3.0] - 2026-06-23

### Added

- Add automatic package manager detection and installation to the GitHub Action — scans for lock files and `packageManager` fields, installs pnpm (via corepack or npm) and bun (via npm) so users no longer need manual setup steps

### Changed

- Bump GitHub Action major version to v3 (`hpehl/depup@v3`) — the action now handles package manager setup automatically, removing the need for `pnpm/action-setup`, `actions/setup-node`, or `oven-sh/setup-bun` steps

## [1.2.1] - 2026-06-22

### Added

- Add short options for common CLI flags: `-d` (dependencies), `-p` (plugins), `-D` (dev dependencies), `-t` (tools), `-m` (maven), `-n` (npm), `-s` (stable), `-M` (managed), `-U` (unmanaged), `-o` (outdated), `-v` (vulnerable)

### Changed

- Rename `--dev-deps` to `--dev-dependencies` for consistent naming with `--dependencies`

## [1.2.0] - 2026-06-22

### Added

- Add npm dependency and dev dependency categories to the GitHub Action — creates separate PRs for outdated npm packages, requires the package manager to be installed on the runner via `actions/setup-node` or similar

### Changed

- Bump GitHub Action major version to v2 (`hpehl/depup@v2`)

## [1.1.6] - 2026-06-19

### Fixed

- Force-push depup branches in GitHub Action to prevent push failures when a previous run left a stale remote branch

## [1.1.5] - 2026-06-19

### Changed

- Include source file path in GitHub Action PR body table

## [1.1.4] - 2026-06-19

### Fixed

- Check npm `packageManager` tool versions even when the package manager binary is not installed (e.g. in CI)
- Include Maven property name in tool version results so the updater can rewrite `<properties>` entries for tools like Node.js and package managers

## [1.1.3] - 2026-06-19

### Fixed

- Configure git identity in the GitHub Action before commit/push to prevent authentication failures

## [1.1.2] - 2026-06-19

### Fixed

- Set `GH_TOKEN` in the install step of the GitHub Action so `gh release view` can resolve the latest version
- Fix Maven updater writing to the wrong POM when a version property is defined in the root POM but referenced in a child POM

## [1.1.0] - 2026-06-19

### Added

- Discover version properties defined in child POMs, not just the root POM
- Discover modules, properties, dependencies, and plugins inside Maven `<profiles>` sections

## [1.0.0] - 2026-06-19

### Added

- Add composite GitHub Action for automatic dependency update PRs — loops over 6 categories (Maven managed/unmanaged deps, Maven managed/unmanaged plugins, Maven tools, npm tools), creates one PR per category with a Markdown table of outdated artifacts
- Add mdBook documentation site deployed to GitHub Pages at [hpehl.github.io/depup](https://hpehl.github.io/depup)

### Changed

- Allow multiple kind filters combined with OR — `--dependencies --plugins` now shows both kinds at once, while all other filters still combine with AND
- Rewrite `action.yml` from generic check/audit runner to single-purpose auto-PR workflow with new inputs (`stable`, `include`, `exclude`, `base-branch`, `labels`)
- Extract shared `fetch_json` helper with 10 MB response size limit, removing HTTP boilerplate from Maven and npm resolvers
- Add symlink protection to POM file reads

### Removed

- Remove `sbom` subcommand (CycloneDX SBOM generation)
- Remove `command`, `args`, and `comment` inputs from the GitHub Action

## [0.3.0] - 2026-06-17

### Added

- Add `sbom` subcommand to generate CycloneDX 1.5 JSON Bills of Materials with Package URL (PURL) identifiers for all discovered dependencies
- Add composite GitHub Action (`action.yml`) for CI integration — installs depup, runs any subcommand, and posts Markdown reports to job summaries and PR comments
- Add `comment` input to the GitHub Action for posting results as PR comments that update in place on re-runs

### Changed

- Use granular exit codes for CI pipelines: 0 = clean, 1 = outdated/errors (check/update), 2 = vulnerabilities found (audit), 3 = critical/high vulnerabilities (audit)

## [0.2.2] - 2026-06-17

### Added

- Add `--vulnerable` flag to the `audit` subcommand to show only dependencies with known vulnerabilities
- Add POM file size limit (10 MB) to reject oversized files before parsing
- Add `#[must_use]` annotations on `find_latest()` and `is_newer()` version functions

### Changed

- Add labeled progress bars across all subcommands with consistent two-phase reporting: "Collecting" for dependency discovery, plus "Updating" or "Auditing" for the action phase
- Extract shared `progress::phase_bar()` helper with label alignment, `▰▱` bar characters, and json-aware hiding
- Keep progress bars visible after completion with a "done" message instead of clearing them
- Replace stringly-typed OSV dependency keys with structured `DepKey` type for type-safe vulnerability lookups
- Add `anyhow::Context` to OSV HTTP calls for clearer error messages on network failures
- Log warnings when OSV vulnerability detail fetches fail instead of silently counting errors
- Canonicalize directory paths before running npm package manager commands to prevent path traversal
- Use `tokio::fs::read_to_string` in bun resolver instead of blocking `std::fs` in async context
- Skip packages with no installed version in bun resolver instead of reporting empty version strings
- Extract shared `skip_element()` helper in POM writer to eliminate duplicated XML element skipping logic
- Narrow `pub(crate)` visibility to `pub(super)` for POM writer internals
- Extract `MAX_PROPERTY_RESOLUTION_DEPTH` constant and log a warning when the depth limit is reached
- Add early return in `resolve_value()` for plain strings that don't contain `${`
- Log warning on failed canonicalization of project root and on malformed `package.json` files

### Removed

- Remove sample output section from README

### Fixed

- Fix double `finish_and_clear()` call on progress bar in Maven updater

## [0.2.1] - 2026-06-16

### Added

- Track `packageManager` version from `package.json` (e.g., `"pnpm@9.15.0"`) as a `ToolVersion` dependency across all subcommands
  - `check`: reports current vs latest version from the npm registry
  - `update`: rewrites the `packageManager` field in `package.json` to the latest version
  - Strips Corepack `+hash` suffixes (e.g., `pnpm@9.15.0+sha512.abc...`)
  - Works with existing filters: `--tools`, `--npm`, `--include`/`--exclude`, `--outdated`

### Changed

- Right-align version numbers in the output column when a property name is present
- Sort tool versions last in output for both Maven and npm ecosystems

### Fixed

- Skip `.pnpm-store/` and other non-project directories during npm discovery to avoid duplicate entries

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
  - Supports all check filters: `--maven`/`--npm`, `--dependencies`/`--plugins`/`--dev-dependencies`, `--managed`/`--unmanaged`, `--include`/`--exclude`
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

[Unreleased]: https://github.com/hpehl/depup/compare/v1.3.0...HEAD
[1.3.0]: https://github.com/hpehl/depup/compare/v1.2.1...v1.3.0
[1.2.1]: https://github.com/hpehl/depup/compare/v1.2.0...v1.2.1
[1.2.0]: https://github.com/hpehl/depup/compare/v1.1.6...v1.2.0
[1.1.6]: https://github.com/hpehl/depup/compare/v1.1.5...v1.1.6
[1.1.5]: https://github.com/hpehl/depup/compare/v1.1.4...v1.1.5
[1.1.4]: https://github.com/hpehl/depup/compare/v1.1.3...v1.1.4
[1.1.3]: https://github.com/hpehl/depup/compare/v1.1.2...v1.1.3
[1.1.2]: https://github.com/hpehl/depup/compare/v1.1.1...v1.1.2
[1.1.0]: https://github.com/hpehl/depup/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/hpehl/depup/compare/v0.3.0...v1.0.0
[0.3.0]: https://github.com/hpehl/depup/compare/v0.2.2...v0.3.0
[0.2.2]: https://github.com/hpehl/depup/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/hpehl/depup/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/hpehl/depup/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/hpehl/depup/releases/tag/v0.1.0
