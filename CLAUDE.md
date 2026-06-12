# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

`mvnup` is a Rust CLI that discovers version properties (`${version.*}`) across multi-module Maven projects and checks them against upstream registries. It solves a gap where Maven's `versions:display-property-updates` fails when properties are defined in a parent POM but referenced in child POMs.

See `VERSION_CHECKER_PLAN.md` for the full design and Phase 2 roadmap (npm/Node.js checks, TOML config).

## Build & Test

```bash
cargo build                    # compile
cargo build --release          # release build (uses LTO)
cargo test                     # all unit + integration tests
cargo test pom::tests          # run tests in a specific module
cargo test -- --nocapture      # show println output during tests
cargo run -- check /path       # check a Maven project
cargo run -- check --json /path  # JSON output mode
cargo run -- completions       # generate shell completions
cargo clippy                   # lint
cargo fmt                      # format
```

The integration test (`tests/discovery_test.rs`) hits Maven Central — it requires network access.

## Architecture

The pipeline flows: **Discovery → Registry Lookup → Comparison → Output**.

### CLI Layer

- **`app.rs`** — Defines the clap `Command` tree using the builder API (not derive macros). Subcommands: `check` (default), `completions`. Global `--json` flag. Styled help text. Separated from `main.rs` so the completion system can build the command tree independently.

- **`main.rs`** — Entry point. Wires `CompleteEnv` for dynamic shell completions, dispatches subcommands, handles top-level error reporting with JSON error envelope support.

- **`args.rs`** — Helper functions to extract typed arguments from clap `ArgMatches`.

- **`constants.rs`** — Static values: Maven Central URL, concurrency limits, HTTP timeout.

### Command Layer (`src/command/`)

- **`check.rs`** — The main check pipeline: discover POM modules, check versions concurrently with progress spinners, sort/filter results, output table or JSON. Uses `tokio::task::JoinSet` for parallel registry checks with semaphore-based rate limiting.

- **`completions.rs`** — Shell completion generation and installation. Supports bash, zsh, fish, elvish, powershell. Auto-detects shell, installs to standard paths.

### Core Layer

- **`pom.rs`** — Parses POM XML using quick-xml's event-based reader (not serde). This is intentional: serde can't handle `<properties>` blocks with arbitrary child element names as a `HashMap<String, String>`. Handles XML namespaces.

- **`discovery.rs`** — Walks the module tree starting from root `pom.xml`, follows `<modules>` declarations recursively. For each artifact with a `${version.*}` version reference, maps it back to the property value in the root POM's `<properties>`. Also collects `<repositories>` and `<pluginRepositories>` from all POMs, deduplicates by URL, and returns them in `DiscoveryResult`.

- **`version.rs`** — Version parsing and comparison. Handles Maven-specific formats like `3.0.0.Final` and `2.1.0-SP1` that don't follow strict semver. A version without a qualifier sorts higher than one with a qualifier (e.g., `1.0.0` > `1.0.0.Final`).

### Registry Layer (`src/registry/`)

- **`mod.rs`** — Defines `CheckResult` struct.

- **`maven.rs`** — Unified Maven repository checker using `maven-metadata.xml`. Tries Maven Central (`repo1.maven.org/maven2`) first; if not found, queries all POM-defined custom repositories in parallel using `JoinSet`. Matches `RepositoryKind::Standard` repos to dependencies and `RepositoryKind::Plugin` repos to plugins. Filters pre-release versions by default (alpha, beta, RC, milestone, SNAPSHOT).

### Output & Error Layer

- **`error.rs`** — Structured error types with `thiserror`. `MvnupError` carries a stable `MvnupErrorCode` for machine consumption and a human-readable message. `JsonErrorEnvelope` provides structured JSON error output when `--json` is active.

- **`json.rs`** — Serializable output types (`JsonResult`) for JSON mode.

- **`output.rs`** — Table (colored via `console` crate) and JSON formatters.

- **`progress.rs`** — Progress bars using `indicatif`. Braille spinner with `MultiProgress` for concurrent checks. Hidden in JSON mode.

## Patterns

These patterns are shared with the `mgt` and `wado` CLI tools:

- **Clap builder API** with styled help text, separated `app.rs` for independent completion tree building
- **Structured errors** with `thiserror`, stable error codes, JSON error envelope
- **Progress bars** via `indicatif` with hidden mode for JSON output
- **`console` crate** for terminal colors and styling (not `colored`)
- **`JoinSet`** for parallel async operations (not `futures::join_all`)
- **Command module organization** — each subcommand in `src/command/`
- **Shell completions** via `clap_complete` with `unstable-dynamic` feature

## Known Quirks

- Maven Central requires a `User-Agent` header or returns 403. The client sets `mvnup/{version}`.
- Artifacts not on Maven Central that also aren't in any POM-defined repository will show as errors. Phase 2 will add a skip list via TOML config.

## Release

Uses `cargo-release` with `release.toml`. Changelog follows [Keep a Changelog](https://keepachangelog.com/) format.
