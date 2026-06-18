# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

`depup` is a Rust CLI that checks and updates dependency versions across multiple ecosystems. It currently supports:

- **Maven** — Discovers version properties (any `${...}` reference, not just `${version.*}`) and plain inline versions across multi-module Maven projects and checks them against Maven Central and custom repositories.
- **npm** — Discovers npm ecosystem projects in a directory tree and checks for outdated packages. Supports multiple package managers: npm, pnpm, yarn (classic), and bun. Auto-detects the package manager by lock file or `packageManager` field in `package.json`.

Auto-detection picks the ecosystem based on project files (`pom.xml` → Maven, lock file or `packageManager` field → npm ecosystem).

## Build & Test

```bash
cargo build                              # compile
cargo build --release                     # release build (uses LTO)
cargo test                                # all unit + integration tests
cargo test maven::pom::tests             # run tests in a specific module
cargo test -- --nocapture                 # show println output during tests
cargo run -- check /path                  # auto-detect ecosystems and check all
cargo run -- check --json /path           # check with JSON output
cargo run -- check --outdated /path       # only show outdated dependencies
cargo run -- update /path                 # update outdated dependencies
cargo run -- audit /path                  # audit for known vulnerabilities
cargo run -- completions                  # generate shell completions
cargo clippy                              # lint
cargo fmt                                 # format
```

## Architecture

The check pipeline flows: **Discovery → Check → Comparison → Output**, with ecosystem-specific discovery and checking. The update pipeline reuses the check pipeline to identify outdated dependencies, then applies updates: **Check → Filter Outdated → Update → Report**. The audit pipeline reuses the check pipeline to collect dependency versions, then queries OSV.dev for known vulnerabilities: **Check → Filter → OSV Batch Query → Fetch Details → Report**. Check, update, and audit use labeled progress bars via `progress::phase_bar()`: check has one phase ("Collecting"), while update ("Collecting" + "Updating") and audit ("Collecting" + "Auditing") have two.

Exit codes are granular for CI integration: 0 = clean, 1 = outdated deps or update errors, 2 = vulnerabilities found, 3 = critical/high vulnerabilities found.

### CLI Layer

- **`app.rs`** — Defines the clap `Command` tree using the builder API (not derive macros). Subcommands: `check`, `update`, `audit`, `completions`. Global `--json` flag. Styled help text. Separated from `main.rs` so the completion system can build the command tree independently.

- **`main.rs`** — Entry point. Wires `CompleteEnv` for dynamic shell completions, dispatches subcommands (`check`, `update`, `audit`, `completions`), handles top-level error reporting with JSON error envelope support. Returns granular exit codes (0/1/2/3) for CI integration.

- **`constants.rs`** — Static values: Maven Central URL, Node.js dist URL, npm registry URL, concurrency limits, HTTP timeout, shared HTTP client factory.

### Command Layer (`src/command/`)

- **`pipeline.rs`** — Shared discovery and version resolution pipeline used by `check`, `update`, and `audit`. Contains `detect_ecosystems()` (shared ecosystem detection from filters + project files) and `resolve_versions()` (discovers Maven via `pom.xml` and npm via lockfiles, resolves all versions concurrently with `JoinSet`, returns `(Vec<CheckResult>, Vec<NpmProject>)`).

- **`check.rs`** — Orchestrates the check subcommand. Calls `pipeline::resolve_versions()`, applies `Filter`, outputs results as table or JSON. Exits with code 1 when outdated dependencies are found.

- **`update.rs`** — Orchestrates the update subcommand. Calls `pipeline::resolve_versions()`, applies `Filter` to select which outdated deps to update, then:
  - Maven: calls `maven::updater::apply_updates()` to rewrite POM files in place, with per-POM progress bar.
  - npm: calls `npm::updater::update_project()` for each project with outdated deps, with per-project progress bar.
  - Supports all check filters (`--managed`, `--dependencies`, `--include`, `--exclude`, etc.) plus `--dry-run`.
  - Output mirrors check: grouped by ecosystem/kind, summary line, timing, exit code 1 on errors.

- **`audit/`** — Audit subcommand module:
  - **`mod.rs`** — Orchestrates the audit subcommand. Calls `pipeline::resolve_versions()` to discover dependencies with versions, filters out tool versions (they aren't registry packages with OSV vulnerability advisories), queries OSV.dev via `osv::audit()`, applies severity and `--vulnerable` filters, outputs results as table or JSON. Same output style as check/update: progress bar, grouped table, summary line, timing. Exit code 2 when vulnerabilities are found, 3 for critical/high.
  - **`osv.rs`** — OSV.dev API client for vulnerability auditing. Queries the batch endpoint (`POST /v1/querybatch`) with dependency coordinates and versions, fetches full vulnerability details from individual endpoints (`GET /v1/vulns/{id}`). Maps `Ecosystem::Maven` to OSV's `"Maven"` and `Ecosystem::Npm` to `"npm"`. Deduplicates queries and vuln IDs. Extracts severity from CVSS scores or ecosystem/database-specific labels. Skips tool versions.

- **`completions.rs`** — Shell completion generation and installation. Supports bash, zsh, fish, elvish, powershell.

### Maven Ecosystem (`src/maven/`)

- **`pom.rs`** — Parses POM XML using quick-xml's event-based reader (not serde). This is intentional: serde can't handle `<properties>` blocks with arbitrary child element names as a `HashMap<String, String>`. Handles XML namespaces.

- **`discovery.rs`** — Walks the module tree starting from root `pom.xml`, follows `<modules>` declarations recursively. For each artifact, extracts the version — either any `${...}` property reference (skipping `${project.*}`) or a plain inline version number. Maps property references back to values in the root POM's `<properties>` (supports chained resolution up to 10 levels). Also collects `<repositories>` and `<pluginRepositories>` from all POMs, deduplicates by URL.

- **`maven_central.rs`** — `MavenVersionResolver`: resolves artifact versions via `maven-metadata.xml`. Tries Maven Central first; if not found, queries custom repositories in parallel. Matches `RepositoryKind::Standard` repos to dependencies and `RepositoryKind::Plugin` repos to plugins. Filters pre-release versions by default.

- **`resolver.rs`** — Orchestrates Maven version resolution. Wraps discovery, builds `ResolveTask` variants (Maven artifact, Node.js version, package manager version), and runs them concurrently with progress reporting.

- **`node.rs`** — `NodeResolver`: resolves Node.js version properties found in Maven POMs (e.g., `version.node`) against the Node.js distribution index.

- **`pom_writer/`** — Surgical POM version updater, split into focused sub-modules:
  - **`mod.rs`** — Shared `Replacement` struct, `apply_replacements()` string splicing, `local_name()` XML helper.
  - **`properties.rs`** — `update_properties()`: replaces values in `<properties>` blocks.
  - **`inline.rs`** — `update_inline_versions()`: replaces `<version>` elements inside dependency/plugin blocks matched by `groupId:artifactId` coordinates. Includes `InlineVersionUpdate` type and predicates for artifact block detection.

- **`updater.rs`** — Bridges version results to POM file writes. Filters to Maven + outdated, groups by source POM. For each POM, applies property updates then inline version updates in sequence. Both managed and unmanaged versions are updated.

- **`pm_versions.rs`** — `PmVersionsResolver`: resolves package manager tool version properties found in Maven POMs (e.g., `version.npm`, `version.pnpm`, `version.yarn`) against the npm registry.

- **`tool.rs`** — `ToolVersionResolver` trait and `ToolResolverRegistry`. Extensible mechanism for resolving non-Maven version properties. Each resolver declares property name patterns it handles.

### npm Ecosystem (`src/npm/`)

- **`mod.rs`** — `PackageManager` enum (Npm, Pnpm, Yarn, Bun), `InstalledPackage` struct, `PackageManagerResolver` trait with `list_packages()`, `outdated_packages()`, and `update_packages()` methods, shared `run_pm_command()` helper and `read_dev_dependency_names()` utility.

- **`resolver.rs`** — Dispatches to the detected PM, runs `list` and `outdated` commands concurrently via `tokio::try_join!`, and merges results into `CheckResult` values.

- **`discovery.rs`** — Walks a directory tree finding npm ecosystem projects. Detects package manager by lock file (`pnpm-lock.yaml`, `package-lock.json`, `yarn.lock`, `bun.lock`/`bun.lockb`) or `packageManager` field in `package.json`. Skips directories in `SKIP_DIRS` (e.g., `node_modules`, `.pnpm-store`, `.yarn`, `.bun`) and workspace members (pnpm: `pnpm-workspace.yaml`, npm/yarn/bun: `workspaces` field).

- **`pm_version_check.rs`** — Checks and updates the `packageManager` version in `package.json`. Queries the npm registry for the latest PM version (`check_pm_version()`), and rewrites the field when updating (`update_pm_version()`). Strips Corepack `+hash` suffixes.

- **`pm_npm.rs`** — npm resolver: `npm list --json` + `npm outdated --json`. Uses shared `read_dev_dependency_names()` to classify dev dependencies.

- **`pm_pnpm.rs`** — pnpm resolver: `pnpm list --json` + `pnpm outdated --format json`.

- **`pm_yarn.rs`** — Yarn classic resolver: parses NDJSON from `yarn list --json` and `yarn outdated --json`. Uses shared `read_dev_dependency_names()`.

- **`pm_bun.rs`** — Bun resolver: reads `package.json` + `node_modules/*/package.json` for versions, `bun outdated --format json` for updates.

- **`updater.rs`** — Orchestrates npm updates by delegating to each project's package manager native update command (`npm update`, `pnpm update`, `yarn upgrade`, `bun update`).

### GitHub Action (`action.yml`)

- **`action.yml`** — Composite GitHub Action for creating automatic pull requests for outdated dependencies. Loops over 6 dependency categories (Maven managed/unmanaged deps, Maven managed/unmanaged plugins, Maven tools, npm tools). For each category: checks for outdated deps, skips if no updates or existing PR exists, creates a branch, runs `depup update`, commits changes, and creates a PR. Inputs: `path`, `version`, `stable`, `include`, `exclude`, `token`, `base-branch`, `labels`. Output: `exit-code` (0=no outdated deps, 1=outdated deps found).

  Categories and their flag mappings:

  | Category | depup flags | Branch name |
  |----------|-----------|-------------|
  | Maven managed deps | `--maven --dependencies --managed` | `depup/maven-managed-dependencies` |
  | Maven unmanaged deps | `--maven --dependencies --unmanaged` | `depup/maven-unmanaged-dependencies` |
  | Maven managed plugins | `--maven --plugins --managed` | `depup/maven-managed-plugins` |
  | Maven unmanaged plugins | `--maven --plugins --unmanaged` | `depup/maven-unmanaged-plugins` |
  | Maven tools | `--maven --tools` | `depup/maven-tools` |
  | npm tools | `--npm --tools` | `depup/npm-tools` |

- **`action-scripts/create-prs.sh`** — Main orchestration script. Loops over categories, builds category-specific flags, runs check+update, creates branches, commits, pushes, and creates PRs via `gh pr create`. Skips categories with no changes or existing open PRs.

- **`action-scripts/build-pr-body.sh`** — PR body generator. Takes check JSON output and category label, generates a Markdown table with artifact names, current versions, and latest versions, plus a footer linking to depup.

### Shared Layer

- **`filter/`** — Post-check result filtering based on CLI flags. Composable filters: ecosystem (`--maven`/`--npm`), kind (`--dependencies`/`--plugins`/`--dev-deps`/`--tools`), `--outdated`, `--stable`, `--managed`/`--unmanaged`, `--include`/`--exclude` glob patterns, `--vulnerable` and `--severity` (audit only). Wildcards use `*` only (no regex).
  - **`mod.rs`** — `Filter` struct (derives `Default`), `KindFilter` enum, `Filter::from_matches()` constructor, `Filter::matches()` predicate.
  - **`glob.rs`** — `glob_matches()` function for `*`-wildcard pattern matching against artifact names.

- **`model/`** — Core types shared across all pipelines. `Ecosystem` enum (Maven, Npm), `DependencyKind` enum (Dependency, Plugin, ToolVersion, NpmDep, NpmDevDep), `Dependency` (artifact + optional property + source), `CommandResult` trait for uniform access across result types. `Dependency.artifact` always holds the display name (Maven coordinates, npm package name, tool label). `Dependency.property` is `Some` only for Maven managed deps backed by a `<properties>` entry.
  - **`mod.rs`** — `Ecosystem`, `DependencyKind`, `Dependency`, `CommandResult` trait.
  - **`check.rs`** — `CheckStatus`/`CheckResult` for the check pipeline.
  - **`update.rs`** — `UpdateStatus`/`UpdateResult` for the update pipeline.
  - **`audit.rs`** — `Severity`/`Vulnerability`/`AuditResult` for the audit pipeline.

- **`version.rs`** — Version parsing and comparison. Handles Maven-specific formats like `3.0.0.Final` and `2.1.0-SP1` that don't follow strict semver.

- **`error.rs`** — Structured error types with `thiserror`. `DepupError` carries a stable `DepupErrorCode` for machine consumption. `JsonErrorEnvelope` provides structured JSON error output when `--json` is active.

- **`output/`** — Terminal and JSON output formatting. Groups results by ecosystem and kind with section headers.
  - **`mod.rs`** — `print_table()` (generic grouped table with summary callback), `print_json()` (pretty-print any `Serialize` value).
  - **`json.rs`** — Serializable output types (`JsonResult`, `UpdateJsonResult`, `AuditJsonResult`) for JSON mode. Converts result types to flat JSON-friendly structs.
  - **`format.rs`** — Column formatting, `truncate_middle_pad()`, `DependencyKind` presentation helpers (`kind_color`, `kind_symbol`, `kind_group_label`) — separated from the data model for clean SoC.
  - **`line.rs`** — `OutputLine` trait with implementations for `CheckResult`, `UpdateResult`, and `AuditResult`. Each provides its own version value and styled status column.
  - **`summary.rs`** — `check_summary()`, `update_summary()`, `audit_summary()` — per-subcommand statistics with kind legend.

- **`progress.rs`** — Progress bars using `indicatif`. `phase_bar(label, total, json)` creates labeled, aligned bars with `▰▱` characters; hidden in JSON mode. Labels are padded to 10 characters for vertical alignment across phases (e.g., "Collecting", "Updating", "Auditing"). Bars persist after completion with a "done" message.

## Patterns

These patterns are shared with the `mgt` and `wado` CLI tools:

- **Clap builder API** with styled help text, separated `app.rs` for independent completion tree building
- **Structured errors** with `thiserror`, stable error codes, JSON error envelope
- **Progress bars** via `indicatif` with hidden mode for JSON output
- **`console` crate** for terminal colors and styling (not `colored`)
- **`JoinSet`** for parallel async operations (not `futures::join_all`)
- **Command module organization** — each subcommand in `src/command/`
- **Shell completions** via `clap_complete` with `unstable-dynamic` feature
- **Trait-based dispatch** — `PackageManagerResolver` trait for PM-specific operations, `ToolVersionResolver` trait for Maven tool-version properties

## Known Quirks

- Maven Central requires a `User-Agent` header or returns 403. The client sets `depup/{version}`.
- Artifacts not on Maven Central that also aren't in any POM-defined repository will show as errors.
- npm ecosystem checks require the respective package manager (npm, pnpm, yarn, or bun) to be installed and on PATH.
- **pnpm catalogs** (`"catalog:<name>"` in `package.json`, defined in `pnpm-workspace.yaml`) are not explicitly supported. Versions are resolved correctly via `pnpm list`/`pnpm outdated`, and updates are handled by `pnpm update`. Explicit support would require rewriting `pnpm-workspace.yaml` — not worth it while catalogs remain pnpm-only and the delegation approach works.

## Installation

Distributed via:

- **Homebrew** — `brew install hpehl/tap/depup` (macOS Intel & Apple Silicon, formula in `hpehl/homebrew-tap`)
- **Cargo** — `cargo install depup-cli` (published to crates.io as `depup-cli`, installs the `depup` binary)
- **GitHub Releases** — precompiled binaries for macOS (x64, arm64), Linux (x64), Windows (x64)
- **Source** — `cargo build --release && cargo install --path .`

## Release

Release process mirrors `wado`:

1. Run `./release.sh <version>` — validates semver, checks clean tree, runs `cargo release` which bumps `Cargo.toml`, updates `CHANGELOG.md`, commits, tags, and pushes.
2. Tag push triggers `.github/workflows/release.yml`:
   - Creates GitHub release with changelog excerpt
   - Publishes crate to crates.io (`CRATES_TOKEN` secret)
   - Builds binaries for 4 targets (linux x64, macOS x64/arm64, Windows x64)
   - Updates `Formula/depup.rb` in `hpehl/homebrew-tap` with new version and SHA256 checksums (`FORMULA_TOKEN` secret)

Uses `cargo-release` with `release.toml`. Changelog follows [Keep a Changelog](https://keepachangelog.com/) format. CI verification via `.github/workflows/verify.yml` on push/PR to main.
