# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

`depup` is a Rust CLI that checks dependency versions across multiple ecosystems. It currently supports:

- **Maven** — Discovers version properties (`${version.*}`) across multi-module Maven projects and checks them against Maven Central and custom repositories.
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
cargo run -- update /path                 # update dependencies (stub)
cargo run -- audit /path                  # audit dependencies (stub)
cargo run -- completions                  # generate shell completions
cargo clippy                              # lint
cargo fmt                                 # format
```

## Architecture

The pipeline flows: **Discovery → Check → Comparison → Output**, with ecosystem-specific discovery and checking.

### CLI Layer

- **`app.rs`** — Defines the clap `Command` tree using the builder API (not derive macros). Subcommands: `check` (auto-detects all ecosystems), `update` (stub), `audit` (stub), `completions`. Global `--json` flag. Styled help text. Separated from `main.rs` so the completion system can build the command tree independently.

- **`main.rs`** — Entry point. Wires `CompleteEnv` for dynamic shell completions, dispatches subcommands (`check`, `update`, `audit`, `completions`), handles top-level error reporting with JSON error envelope support.

- **`constants.rs`** — Static values: Maven Central URL, Node.js dist URL, npm registry URL, concurrency limits, HTTP timeout, shared HTTP client factory.

### Command Layer (`src/command/`)

- **`mod.rs`** — Shared `not_implemented()` helper for stub subcommands (used by `update` and `audit`).

- **`check.rs`** — Orchestrates check pipelines across all ecosystems:
  - `check()` — Discovers all ecosystems in the target path (Maven if `pom.xml` exists, npm if lockfile or `packageManager` field exists), runs all found, merges results.
  - `spawn_npm_checks()` — Extracted helper that spawns npm project checks concurrently with semaphore-based rate limiting.
  - Both ecosystems use `tokio::task::JoinSet` for parallel checks.
  - Output: combined table or JSON across all ecosystems.

- **`update.rs`** / **`audit.rs`** — Stubs delegating to `not_implemented()`.

- **`completions.rs`** — Shell completion generation and installation. Supports bash, zsh, fish, elvish, powershell.

### Maven Ecosystem (`src/maven/`)

- **`pom.rs`** — Parses POM XML using quick-xml's event-based reader (not serde). This is intentional: serde can't handle `<properties>` blocks with arbitrary child element names as a `HashMap<String, String>`. Handles XML namespaces.

- **`discovery.rs`** — Walks the module tree starting from root `pom.xml`, follows `<modules>` declarations recursively. For each artifact with a `${version.*}` version reference, maps it back to the property value in the root POM's `<properties>`. Also collects `<repositories>` and `<pluginRepositories>` from all POMs, deduplicates by URL.

- **`maven_central.rs`** — Unified Maven repository checker using `maven-metadata.xml`. Tries Maven Central first; if not found, queries custom repositories in parallel. Matches `RepositoryKind::Standard` repos to dependencies and `RepositoryKind::Plugin` repos to plugins. Filters pre-release versions by default.

- **`checker.rs`** — Orchestrates Maven checks. Wraps discovery, builds `CheckTask` variants (Maven artifact, Node.js version, package manager version), and runs them concurrently with progress reporting.

- **`node.rs`** — Checks Node.js version properties found in Maven POMs (e.g., `version.node`) against the Node.js distribution index.

- **`pm_versions.rs`** — Checks package manager tool version properties found in Maven POMs (e.g., `version.npm`, `version.pnpm`, `version.yarn`) against the npm registry.

### npm Ecosystem (`src/npm/`)

- **`mod.rs`** — `PackageManager` enum (Npm, Pnpm, Yarn, Bun), `PackageManagerChecker` trait, shared `read_dev_dependency_names()` utility. Each PM implements `list_packages()` and `outdated_packages()` via the trait.

- **`checker.rs`** — Dispatches to the detected PM, runs `list` and `outdated` commands concurrently via `tokio::try_join!`, and merges results into `CheckResult` values.

- **`discovery.rs`** — Walks a directory tree finding npm ecosystem projects. Detects package manager by lock file (`pnpm-lock.yaml`, `package-lock.json`, `yarn.lock`, `bun.lock`/`bun.lockb`) or `packageManager` field in `package.json`. Skips `node_modules/`, workspace members (pnpm: `pnpm-workspace.yaml`, npm/yarn/bun: `workspaces` field).

- **`pm_npm.rs`** — npm checker: `npm list --json` + `npm outdated --json`. Uses shared `read_dev_dependency_names()` to classify dev dependencies.

- **`pm_pnpm.rs`** — pnpm checker: `pnpm list --json` + `pnpm outdated --format json`.

- **`pm_yarn.rs`** — Yarn classic checker: parses NDJSON from `yarn list --json` and `yarn outdated --json`. Uses shared `read_dev_dependency_names()`.

- **`pm_bun.rs`** — Bun checker: reads `package.json` + `node_modules/*/package.json` for versions, `bun outdated --format json` for updates.

### Shared Layer

- **`registry.rs`** — Core types: `Ecosystem` enum (Maven, Npm), `CheckerKind` enum (Dependency, Plugin, ToolVersion, NpmDep, NpmDevDep), `CheckId` struct (groups identity fields: ecosystem, kind, property name, artifact, source, has_version_property), `CheckStatus` enum (UpToDate, Outdated, Skipped, Error), and `CheckResult` struct combining `CheckId`, current version, and `CheckStatus`. Factory methods: `checked()`, `skipped()`, `error()`. Accessor methods for all fields.

- **`version.rs`** — Version parsing and comparison. Handles Maven-specific formats like `3.0.0.Final` and `2.1.0-SP1` that don't follow strict semver.

- **`error.rs`** — Structured error types with `thiserror`. `DepupError` carries a stable `DepupErrorCode` for machine consumption. `JsonErrorEnvelope` provides structured JSON error output when `--json` is active.

- **`json.rs`** — Serializable output types (`JsonResult`) for JSON mode. Converts `CheckResult` to a flat JSON-friendly struct.

- **`output.rs`** — Summary table (colored via `console` crate) and JSON formatters. Groups results by ecosystem and kind with section headers.

- **`progress.rs`** — Progress bars using `indicatif`. Block-style bar with `MultiProgress` for concurrent checks. Hidden in JSON mode.

## Patterns

These patterns are shared with the `mgt` and `wado` CLI tools:

- **Clap builder API** with styled help text, separated `app.rs` for independent completion tree building
- **Structured errors** with `thiserror`, stable error codes, JSON error envelope
- **Progress bars** via `indicatif` with hidden mode for JSON output
- **`console` crate** for terminal colors and styling (not `colored`)
- **`JoinSet`** for parallel async operations (not `futures::join_all`)
- **Command module organization** — each subcommand in `src/command/`
- **Shell completions** via `clap_complete` with `unstable-dynamic` feature
- **Trait-based dispatch** — `PackageManagerChecker` trait for PM-specific operations, `ToolVersionChecker` trait for Maven tool-version properties

## Known Quirks

- Maven Central requires a `User-Agent` header or returns 403. The client sets `depup/{version}`.
- Artifacts not on Maven Central that also aren't in any POM-defined repository will show as errors.
- npm ecosystem checks require the respective package manager (npm, pnpm, yarn, or bun) to be installed and on PATH.

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
