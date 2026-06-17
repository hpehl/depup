# depup

Check dependency versions across multiple ecosystems.

`depup` auto-detects project ecosystems in a directory tree and checks all dependencies for newer versions. It supports **Maven** and **npm** (with npm, pnpm, yarn classic, and bun package managers).

## Installation

[Precompiled binaries](https://github.com/hpehl/depup/releases) are available for macOS (Intel & Apple Silicon), Linux, and Windows.

### Brew

```shell
brew tap hpehl/tap
brew install depup
```

### Cargo

```shell
cargo install depup-cli
```

### Build from source

1. [Install Rust and Cargo](https://www.rust-lang.org/tools/install)
2. `git clone git@github.com:hpehl/depup.git`
3. `cd depup`
4. `cargo build --release && cargo install --path .`

This installs the `depup` binary to `~/.cargo/bin/` which should be in your `$PATH`.

## Usage

If both Maven and npm ecosystem projects are found in the target path, both are processed and results are combined.

### Check

```bash
# Check current directory (auto-detects ecosystems)
depup check

# Check a specific project
depup check /path/to/project

# JSON output (for scripting)
depup check --json

# Only show outdated versions
depup check --outdated

# Exclude pre-release versions (alpha, beta, RC, milestone)
depup check --stable

# Filter by ecosystem
depup check --maven
depup check --npm

# Filter by kind
depup check --dependencies
depup check --plugins
depup check --dev-deps
depup check --tools

# Filter by version property (Maven only)
depup check --managed          # only dependencies using a version property
depup check --unmanaged        # only dependencies with plain inline versions

# Filter by artifact name (glob wildcards)
depup check --include 'org.junit:*'           # only org.junit artifacts
depup check --exclude '*:guava'               # exclude guava
depup check --include 'org.wildfly:*' --exclude '*:core'  # combine filters
```

### Update

```bash
# Update all outdated dependencies
depup update

# Update a specific project
depup update /path/to/project

# Preview what would be updated (no changes made)
depup update --dry-run

# Only update to stable releases (exclude pre-release versions)
depup update --stable

# Filter by ecosystem
depup update --maven
depup update --npm

# Filter by kind
depup update --dependencies
depup update --plugins
depup update --dev-deps
depup update --tools

# Filter by version property (Maven only)
depup update --managed          # only update managed version properties
depup update --unmanaged        # only update inline versions

# Filter by artifact name (glob wildcards)
depup update --include 'org.junit:*'           # only update org.junit artifacts
depup update --exclude '*:guava'               # skip guava
depup update --include 'react*'                # only update react packages (npm)

# JSON output
depup update --json
```

For Maven, `depup update` rewrites version numbers in POM files while preserving all formatting, comments, and indentation. Both managed properties (`${...}` references in `<properties>` blocks) and plain inline versions (`<version>5.10.0</version>` inside dependency/plugin blocks) are updated.

For npm, `depup update` delegates to the detected package manager's native update command (`npm update`, `pnpm update`, `yarn upgrade`, `bun update`).

The exit code is `0` when all updates succeed, `1` when any update fails (see [Exit Codes](#exit-codes)).

### Audit

```bash
# Audit all dependencies for known vulnerabilities
depup audit

# Audit a specific project
depup audit /path/to/project

# JSON output
depup audit --json

# Filter by minimum severity level
depup audit --severity critical      # only critical vulnerabilities
depup audit --severity high          # critical + high

# Filter by ecosystem
depup audit --maven
depup audit --npm

# Filter by kind
depup audit --dependencies
depup audit --plugins
depup audit --dev-deps

# Filter by version property (Maven only)
depup audit --managed
depup audit --unmanaged

# Filter by artifact name (glob wildcards)
depup audit --include 'org.wildfly:*'
depup audit --exclude '*:guava'
```

The audit subcommand queries [OSV.dev](https://osv.dev/) for known vulnerabilities in all discovered dependencies. It works for both Maven and npm ecosystems using the same unified API. Tool versions (Node.js, package manager versions) are excluded — they aren't registry packages with OSV vulnerability advisories, so the `--tools` filter is not available for audit.

The exit code is `0` when no vulnerabilities are found, `2` when vulnerabilities are detected, or `3` when critical or high severity vulnerabilities are found (see [Exit Codes](#exit-codes)).

### SBOM

```bash
# Generate a CycloneDX 1.5 SBOM to stdout
depup sbom

# Generate SBOM for a specific project
depup sbom /path/to/project

# Write SBOM to a file
depup sbom -o bom.json

# Filter by ecosystem
depup sbom --maven
depup sbom --npm

# Filter by kind
depup sbom --dependencies
depup sbom --plugins
depup sbom --dev-deps

# Filter by artifact name (glob wildcards)
depup sbom --include 'org.wildfly:*'
depup sbom --exclude '*:guava'
```

The sbom subcommand generates a [CycloneDX](https://cyclonedx.org/) 1.5 JSON Bill of Materials from all discovered dependencies. Output includes [Package URL](https://github.com/package-url/purl-spec) (PURL) identifiers for each component. Tool versions (Node.js, package manager versions) are excluded.

### Completions

```bash
# Generate shell completions (auto-detects shell)
depup completions

# Install shell completions
depup completions --install

# Generate completions for a specific shell
depup completions fish
```

## Subcommands

| Command | Description |
|---------|-------------|
| `check` | Check dependencies for newer versions |
| `update` | Update outdated dependencies in place |
| `audit` | Audit dependencies for known vulnerabilities via [OSV.dev](https://osv.dev/) |
| `sbom` | Generate a CycloneDX 1.5 SBOM (Software Bill of Materials) |
| `completions` | Generate and install shell completions |

## Ecosystems

### Maven

Scans multi-module Maven projects and checks dependency versions against upstream Maven repositories. Discovers:

- **Property references** — any `${...}` property used as a version (e.g., `${junit.version}`, `${version.wildfly}`, `${my.lib.version}`). The only exclusion is `${project.*}` properties which are Maven built-ins.
- **Plain inline versions** — artifacts with hardcoded version numbers (e.g., `<version>5.10.0</version>`) are also checked.
- **Tool versions** — Node.js and package manager version properties in Maven POMs (e.g., `version.node`, `version.npm`, `version.pnpm`, `version.yarn`).

Works where Maven's `versions:display-property-updates` fails — when properties are defined in a parent POM but referenced in child POMs.

### npm

Discovers npm ecosystem projects in the directory tree by detecting the package manager via lock file (`pnpm-lock.yaml`, `package-lock.json`, `yarn.lock`, `bun.lock`/`bun.lockb`) or the `packageManager` field in `package.json`. Runs the appropriate package manager's outdated command on each discovered project and aggregates results. Workspace members are skipped — only root projects are checked.

Supported package managers: **npm**, **pnpm**, **yarn** (classic), **bun**.

> **Note:** pnpm [catalogs](https://pnpm.io/catalogs) (`"catalog:<name>"` version specifiers defined in `pnpm-workspace.yaml`) are resolved transparently by pnpm's own commands — depup does not need to handle them explicitly.

## JSON Mode

Use `--json` for machine-readable output. Progress bars are suppressed, and errors produce structured JSON:

```json
{"error": {"code": "POM_NOT_FOUND", "message": "No pom.xml found in /nonexistent"}}
```

Error codes: `POM_NOT_FOUND`, `POM_PARSE_FAILED`, `HTTP_REQUEST_FAILED`, `CLAP_PARSE_ERROR`, `INTERNAL`.

## How It Works

### Maven

1. Parses the root `pom.xml` and recursively follows `<modules>` declarations
2. For every `<dependency>` and `<plugin>`, extracts the version — either a `${...}` property reference (any name, not just `version.*`) or a plain inline version number
3. Resolves property values from the root POM's `<properties>` block (supports chained references up to 10 levels)
4. Queries Maven Central for the latest version of each artifact (via `maven-metadata.xml`)
5. If not found on Maven Central, queries all `<repositories>` and `<pluginRepositories>` defined in the POMs in parallel
6. Compares versions using Maven-aware ordering (handles `.Final`, `-SP1`, and other qualifiers)

### npm

1. Walks the directory tree finding directories with a recognized lock file or `packageManager` field in `package.json`
2. Auto-detects the package manager (npm, pnpm, yarn, or bun) from the lock file type or `packageManager` field
3. Skips `node_modules/` and workspace members
4. Runs each package manager's list and outdated commands in JSON mode
5. Parses and aggregates results across all discovered projects

## Version Filtering

By default, `depup` includes pre-release versions but always excludes SNAPSHOTs (Maven). Use `--stable` (alias `--releases-only`) to also exclude pre-release versions matching these patterns:

- `*-alpha*`, `*-beta*`
- `*-RC*`, `*-CR*`
- `*-M*` (milestones)
- `*-preview*`, `*-dev*`, `*-incubating*`

SNAPSHOTs are always excluded regardless of flags.

## Shell Completions

Generate and install shell completions for tab-completion of subcommands and flags:

```bash
depup completions --install       # auto-detect shell, install to standard path
depup completions fish            # print fish completions to stdout
depup completions --install zsh   # install zsh completions
```

Supported shells: bash, zsh, fish, elvish, powershell.

## Exit Codes

`depup` uses granular exit codes for CI/CD integration:

| Code | Meaning | Subcommand |
|------|---------|------------|
| 0 | All clean — no issues found | all |
| 1 | Outdated dependencies found, or update errors occurred | check, update |
| 2 | Vulnerabilities found (any severity) | audit |
| 3 | Critical or high severity vulnerabilities found | audit |

CI pipelines can react to specific conditions:

```bash
depup audit --json /path/to/project
case $? in
  0) echo "Clean" ;;
  2) echo "Vulnerabilities found (review recommended)" ;;
  3) echo "Critical/high vulnerabilities — blocking merge" ; exit 1 ;;
esac
```

## GitHub Action

`depup` includes a composite GitHub Action for CI integration. It installs `depup` from GitHub Releases, runs the requested subcommand with `--json`, and posts a Markdown report to the GitHub Actions job summary.

### Check for outdated dependencies on every PR

```yaml
# .github/workflows/depup.yml
name: Dependency Check

on: [pull_request]

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
      - uses: hpehl/depup@v0
        with:
          command: check
          args: '--outdated'
          comment: 'true'
```

When `comment` is `true` and the workflow runs on a `pull_request` event, the report is posted as a PR comment in addition to the job summary. Re-runs update the existing comment in place instead of creating new ones.

### Audit with severity gating

The action always succeeds so the report is generated, but you can check the exit code in a subsequent step to fail the build selectively:

```yaml
jobs:
  audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
      - uses: hpehl/depup@v0
        id: audit
        with:
          command: audit
          args: '--severity high'
      - name: Fail on critical/high vulnerabilities
        if: steps.audit.outputs.exit-code == '3'
        run: exit 1
```

### Generate an SBOM

```yaml
      - uses: hpehl/depup@v0
        with:
          command: sbom
```

### Inputs

| Input | Default | Description |
|-------|---------|-------------|
| `command` | `check` | Subcommand to run: `check`, `audit`, or `sbom` |
| `path` | `.` | Path to the project root |
| `args` | | Additional CLI arguments (e.g., `--outdated`, `--maven`, `--severity critical`) |
| `version` | `latest` | depup version to install (e.g., `0.3.0`) |
| `comment` | `false` | Post results as a PR comment (only on `pull_request` events) |
| `token` | `github.token` | GitHub token for PR comment posting |

### Outputs

| Output | Description |
|--------|-------------|
| `exit-code` | The depup exit code (`0`/`1`/`2`/`3` — see [Exit Codes](#exit-codes)) |
| `json` | Path to the raw JSON output file for use in subsequent steps |

### How it works

1. Downloads the `depup` binary for the runner's OS and architecture from GitHub Releases
2. Runs `depup <command> --json <args> <path>` and captures the output and exit code
3. Formats the JSON output as a Markdown table using `jq` and writes it to `$GITHUB_STEP_SUMMARY`

### Limitations

- The format script requires bash and `jq`, so it works on `ubuntu-latest` and `macos-latest` but not `windows-latest`.

## Requirements

- Rust 1.85+ (edition 2024)
- Network access to Maven Central (`repo1.maven.org`) and any custom repositories defined in the project's POMs
- For npm ecosystem checks: the respective package manager (npm, pnpm, yarn, or bun) must be installed and on PATH

## License

Apache License 2.0
