# mvnup — Implementation Status

## Phase 1: POM Discovery + Maven Central (COMPLETE)

All features implemented and tested (20 tests passing).

### What's Done

- **POM parsing** (`src/pom.rs`) — Event-based XML parser using quick-xml. Extracts `<properties>`, `<modules>`, dependencies, and plugins from all management and direct declaration sections. Handles XML namespaces.
- **Multi-module discovery** (`src/discovery.rs`) — Recursively walks `<modules>` declarations, maps `${version.*}` references to artifact coordinates (groupId + artifactId), resolves property values from root POM.
- **Maven Central lookup** (`src/registry/maven.rs`) — Solr API client using `core=gav` endpoint. Filters SNAPSHOT and pre-release versions by default. Configurable via `--include-pre-releases` flag.
- **Version comparison** (`src/version.rs`) — Handles semver, Maven qualifiers (`.Final`, `-SP1`), `v` prefixes. No-qualifier > with-qualifier ordering.
- **Output** (`src/output.rs`) — Colored table with summary line, JSON output mode.
- **CLI** (`src/main.rs`) — clap-based with flags: `--json`, `--outdated`, `--include-pre-releases`, `-v/--verbose`.
- **Tests** — 19 unit tests across all modules + 1 integration test against Maven Central with a multi-module fixture project.

### Known Limitations (to address in Phase 2)

- No way to skip properties (e.g., `version.kie.j2cl.bom` which is on a custom repo)
- No npm registry checks (needed for PatternFly, corepack)
- No Node.js version checks
- Artifacts not on Maven Central show as errors with no workaround
- Registry lookups are sequential (works fine but could be faster with bounded concurrency)

---

## Phase 2: Non-Maven Rules + TOML Config (NOT STARTED)

### Planned Features

1. **TOML config file** (`mvnup.toml`)
   - Skip list: properties to exclude from checking
   - Custom npm-based version checks
   - Custom Node.js version check
   - Example:
     ```toml
     [skip]
     properties = ["version.kie.j2cl.bom"]

     [[npm]]
     property = "version.patternfly"
     package = "@patternfly/patternfly"

     [[npm]]
     property = "version.corepack"
     package = "corepack"

     [[node]]
     property = "version.node"
     ```

2. **npm registry checker** (`src/registry/npm.rs`)
   - Query `https://registry.npmjs.org/{package}/latest`
   - Implement `VersionChecker` trait

3. **Node.js version checker** (`src/registry/node.rs`)
   - Query Node.js releases API
   - Strip `v` prefix for comparison

4. **Config file loading**
   - New `src/config.rs` module
   - Look for `mvnup.toml` in project root (or `--config` flag)
   - Parse with `toml` crate (add to Cargo.toml)
   - Apply skip list before registry lookups
   - Create non-Maven `ArtifactMapping` entries from config rules

5. **Parallel registry lookups**
   - Use `tokio::spawn` with a `Semaphore` (cap ~10 concurrent requests)
   - Requires making `VersionChecker` implementations `'static` (wrap in `Arc`)

### Implementation Order

1. Config file parsing + skip list (immediate value, simple)
2. npm registry checker
3. Node.js version checker
4. Parallel lookups (performance improvement)
