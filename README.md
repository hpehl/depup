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

`depup` includes a composite GitHub Action that automatically creates pull requests for outdated dependencies. This complements GitHub's Dependabot by handling what Dependabot cannot:

- **Maven property-based versions** — `<properties>` entries like `version.junit` that drive multiple dependencies
- **Tool version properties** — Node.js, npm, pnpm, yarn versions managed in Maven POMs
- **Custom Maven repositories** — artifacts not on Maven Central
- **npm `packageManager` field** — the `"packageManager": "pnpm@9.15.0"` field in `package.json`

The action creates one PR per dependency category (6 categories: Maven managed/unmanaged deps, Maven managed/unmanaged plugins, Maven tools, npm tools). It skips categories with no outdated dependencies or existing open PRs.

### Minimal example

Check everything weekly and create PRs for all categories:

```yaml
name: depup
on:
  schedule:
    - cron: '0 6 * * 1'  # Weekly on Monday at 6am
  workflow_dispatch:

permissions:
  contents: write
  pull-requests: write

jobs:
  update:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: hpehl/depup@v1
```

### Only stable versions, exclude test libraries

```yaml
- uses: hpehl/depup@v1
  with:
    stable: true
    exclude: 'org.junit:*,org.mockito:*'
```

### Only specific artifacts

```yaml
- uses: hpehl/depup@v1
  with:
    include: 'org.wildfly:*,org.jboss:*'
```

### Inputs

| Input | Default | Description |
|-------|---------|-------------|
| `path` | `.` | Path to the project root |
| `version` | `latest` | depup version to install (e.g., `0.4.0`) |
| `stable` | `false` | Exclude pre-release versions (alpha, beta, RC, milestone) |
| `include` | | Only include Maven artifacts matching glob patterns (comma-separated). Exclude takes precedence over include. |
| `exclude` | | Exclude Maven artifacts matching glob patterns (comma-separated). Takes precedence over include. |
| `token` | `github.token` | GitHub token for creating PRs and branches |
| `base-branch` | | Branch to create PRs against (defaults to repository default branch) |
| `labels` | `dependencies` | Comma-separated PR labels (must already exist in the repo) |

### Outputs

| Output | Description |
|--------|-------------|
| `exit-code` | Action exit code: 0=no outdated deps, 1=outdated deps found |

### How it works

The action loops over 6 dependency categories, creating one PR per category when outdated dependencies are found:

| Category | Branch Name |
|----------|-------------|
| Maven managed dependencies | `depup/maven-managed-dependencies` |
| Maven unmanaged dependencies | `depup/maven-unmanaged-dependencies` |
| Maven managed plugins | `depup/maven-managed-plugins` |
| Maven unmanaged plugins | `depup/maven-unmanaged-plugins` |
| Maven tool versions | `depup/maven-tools` |
| npm packageManager versions | `depup/npm-tools` |

For each category:

1. **Check** — run `depup check --json --outdated` with category-specific flags
2. **Skip if empty** — if no outdated dependencies in this category, move to next
3. **Skip if PR exists** — check for open PRs on the category's branch, skip if found
4. **Create branch** — `git checkout -b depup/<category>` from base branch
5. **Update** — run `depup update` to edit files in the working tree
6. **Commit & push** — commit changes and push to origin
7. **Create PR** — `gh pr create` with title, body, and labels
8. **Reset** — return to base branch and delete local branch

PR titles follow the format `chore(deps): bump <category-label>`. PR bodies contain a table with artifact names, current versions, and latest versions. PRs are identifiable by the `depup/` branch prefix and the `dependencies` label (or custom labels specified in the `labels` input).

## Requirements

- Rust 1.85+ (edition 2024)
- Network access to Maven Central (`repo1.maven.org`) and any custom repositories defined in the project's POMs
- For npm ecosystem checks: the respective package manager (npm, pnpm, yarn, or bun) must be installed and on PATH

## License

Apache License 2.0
