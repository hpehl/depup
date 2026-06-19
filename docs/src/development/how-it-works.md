# How It Works

This page describes the internal pipelines that power `depup`.

## Check Pipeline

The check pipeline flows: **Discovery → Resolution → Comparison → Output**.

### Maven

1. **Parse** — reads the root `pom.xml` and recursively follows `<modules>` declarations
2. **Extract versions** — for every `<dependency>` and `<plugin>`, extracts the version:
   - `${...}` property references (any name except `${project.*}`)
   - Plain inline version numbers
3. **Resolve properties** — resolves property values from `<properties>` blocks across all POMs (root and child, root wins on conflict), supporting chained references up to 10 levels
4. **Query upstream** — fetches `maven-metadata.xml` from Maven Central for the latest version of each artifact
5. **Fallback to custom repos** — if not found on Maven Central, queries all `<repositories>` and `<pluginRepositories>` defined in the POMs in parallel
6. **Compare** — compares versions using Maven-aware ordering (handles `.Final`, `-SP1`, and other qualifiers)

### npm

1. **Walk** — traverses the directory tree finding directories with a recognized lock file or `packageManager` field
2. **Detect** — auto-detects the package manager from the lock file type or `packageManager` field
3. **Skip** — skips `node_modules/` and workspace members
4. **Query** — runs each package manager's `list` and `outdated` commands in JSON mode
5. **Aggregate** — parses and merges results across all discovered projects

## Update Pipeline

The update pipeline reuses the check pipeline to identify outdated dependencies, then applies updates: **Check → Filter Outdated → Update → Report**.

### Maven

For each POM file containing outdated versions:

1. Applies **property updates** — replaces values in `<properties>` blocks
2. Applies **inline version updates** — replaces `<version>` elements inside dependency/plugin blocks, matched by `groupId:artifactId` coordinates

The updater is surgical: it only modifies version text while preserving all formatting, comments, and indentation.

### npm

For each project with outdated dependencies, delegates to the detected package manager's native update command. The `packageManager` field in `package.json` is also updated if a newer version is available.

## Audit Pipeline

The audit pipeline reuses the check pipeline to collect dependency versions, then queries for vulnerabilities: **Check → Filter → OSV Batch Query → Fetch Details → Report**.

1. **Collect** — runs the standard check pipeline to discover all dependencies with versions
2. **Filter** — removes tool versions (they aren't registry packages with OSV vulnerability advisories)
3. **Batch query** — sends all dependency coordinates and versions to the [OSV.dev batch endpoint](https://osv.dev/docs/) (`POST /v1/querybatch`)
4. **Fetch details** — for each match, fetches full vulnerability information from `GET /v1/vulns/{id}`
5. **Extract severity** — determines severity from CVSS scores or ecosystem-specific labels
6. **Deduplicate** — removes duplicate vulnerability IDs across dependencies
7. **Report** — groups results by ecosystem and kind

## Progress Reporting

All subcommands use labeled progress bars via `indicatif`:

- **check** — one phase: "Collecting"
- **update** — two phases: "Collecting" + "Updating"
- **audit** — two phases: "Collecting" + "Auditing"

Progress bars are suppressed in JSON mode. Labels are padded to 10 characters for vertical alignment.

## Concurrency

- Maven artifact resolution runs concurrently using `tokio::JoinSet`
- Custom repository queries run in parallel
- npm `list` and `outdated` commands run concurrently per project via `tokio::try_join!`
