# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

`mvnup` is a Rust CLI that discovers version properties (`${version.*}`) across multi-module Maven projects and checks them against upstream registries. It solves a gap where Maven's `versions:display-property-updates` fails when properties are defined in a parent POM but referenced in child POMs.

See `VERSION_CHECKER_PLAN.md` for the full design and Phase 2 roadmap (npm/Node.js checks, TOML config).

## Build & Test

```bash
cargo build                    # compile
cargo test                     # all unit + integration tests
cargo test pom::tests          # run tests in a specific module
cargo test -- --nocapture      # show println output during tests
cargo run -- /path/to/project  # run against a Maven project
cargo run -- --json /path      # JSON output mode
```

The integration test (`tests/discovery_test.rs`) hits Maven Central — it requires network access.

## Architecture

The pipeline flows: **Discovery → Registry Lookup → Comparison → Output**.

- **`pom.rs`** — Parses POM XML using quick-xml's event-based reader (not serde). This is intentional: serde can't handle `<properties>` blocks with arbitrary child element names as a `HashMap<String, String>`. Handles XML namespaces.

- **`discovery.rs`** — Walks the module tree starting from root `pom.xml`, follows `<modules>` declarations recursively. For each artifact with a `${version.*}` version reference, maps it back to the property value in the root POM's `<properties>`.

- **`registry/mod.rs`** — Defines the `VersionChecker` trait. Each registry backend implements `async fn check(&self, mapping: &ArtifactMapping) -> Result<CheckResult>`.

- **`registry/maven.rs`** — Maven Central Solr API client. Uses the `core=gav` endpoint for version enumeration. Filters pre-release versions by default (alpha, beta, RC, milestone, SNAPSHOT).

- **`version.rs`** — Version parsing and comparison. Handles Maven-specific formats like `3.0.0.Final` and `2.1.0-SP1` that don't follow strict semver. A version without a qualifier sorts higher than one with a qualifier (e.g., `1.0.0` > `1.0.0.Final`).

- **`output.rs`** — Table (colored) and JSON formatters.

## Known Quirks

- Maven Central requires a `User-Agent` header or returns 403. The client sets `mvnup/{version}`.
- Use `reqwest::Client::query()` for Maven Central parameters — string-interpolated URLs double-encode the quotes in the Solr query.
- Artifacts not on Maven Central (e.g., org.gwtproject, some Sonatype plugins) will show as errors. Phase 2 will add a skip list via TOML config.
