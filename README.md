# depup

Check dependency versions across multiple ecosystems.

`depup` auto-detects project ecosystems in a directory tree and checks all dependencies for newer versions. Currently supports **Maven** and **npm** (with npm, pnpm, yarn classic, and bun package managers).

## Installation

```bash
cargo install --path .
```

## Usage

```bash
# Check current directory (auto-detects ecosystems)
depup

# Check a specific project
depup check /path/to/project

# JSON output (for scripting)
depup check --json

# Only show outdated versions
depup check --outdated

# Include pre-release versions (Maven only)
depup check --include-pre-releases

# Generate shell completions (auto-detects shell)
depup completions

# Install shell completions
depup completions --install

# Generate completions for a specific shell
depup completions fish
```

The `check` subcommand is the default — `depup /path` is equivalent to `depup check /path`.

If both Maven and npm ecosystem projects are found in the target path, both are checked and results are combined.

## Subcommands

| Command | Description |
|---------|-------------|
| `check` | Check dependencies for newer versions (default) |
| `update` | Update dependencies to their latest versions (not yet implemented) |
| `audit` | Audit dependencies for known vulnerabilities (not yet implemented) |
| `completions` | Generate and install shell completions |

## Ecosystems

### Maven

Scans multi-module Maven projects, discovers all `${version.*}` properties and the artifacts they control, then checks each against upstream Maven repositories. Works where Maven's `versions:display-property-updates` fails — when properties are defined in a parent POM but referenced in child POMs.

Also detects Node.js and package manager version properties in Maven POMs (e.g., `version.node`, `version.npm`, `version.pnpm`, `version.yarn`).

### npm

Discovers npm ecosystem projects in the directory tree by detecting the package manager via lock file (`pnpm-lock.yaml`, `package-lock.json`, `yarn.lock`, `bun.lock`/`bun.lockb`) or the `packageManager` field in `package.json`. Runs the appropriate package manager's outdated command on each discovered project and aggregates results. Workspace members are skipped — only root projects are checked.

Supported package managers: **npm**, **pnpm**, **yarn** (classic), **bun**.

## Example Output

```
  [2/4] ████████████████████████████▓░ org.junit.jupiter:junit-jupiter

  Dependencies
  ✓ org.apache.maven.plugins:maven-compiler-plugin  3.13.0    up-to-date
  → org.junit.jupiter:junit-jupiter                 5.10.0    → 5.12.2
  Plugins
  ✓ org.apache.maven.plugins:maven-javadoc-plugin   3.12.0    up-to-date
  ✓ org.mockito:mockito-core                        5.18.0    up-to-date

4 checked: 3 current, 1 outdated  (● Dependency, ■ Plugin)

Done in 1s
```

The exit code is `0` when all versions are current, `1` when any are outdated.

## JSON Mode

Use `--json` for machine-readable output. Progress bars are suppressed, and errors produce structured JSON:

```json
{"error": {"code": "POM_NOT_FOUND", "message": "No pom.xml found in /nonexistent"}}
```

Error codes: `POM_NOT_FOUND`, `POM_PARSE_FAILED`, `REGISTRY_LOOKUP_FAILED`, `NO_VERSIONS_FOUND`, `HTTP_REQUEST_FAILED`, `CLAP_PARSE_ERROR`, `INTERNAL`.

## How It Works

### Maven

1. Parses the root `pom.xml` and recursively follows `<modules>` declarations
2. For every `<dependency>` and `<plugin>` using `${version.*}`, maps the property to its groupId and artifactId
3. Resolves property values from the root POM's `<properties>` block
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

By default, `depup` excludes pre-release versions and SNAPSHOTs (Maven). Use `--include-pre-releases` to also show pre-release versions matching these patterns:

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

## Requirements

- Rust 1.85+ (edition 2024)
- Network access to Maven Central (`repo1.maven.org`) and any custom repositories defined in the project's POMs
- For npm ecosystem checks: the respective package manager (npm, pnpm, yarn, or bun) must be installed and on PATH

## License

Apache License 2.0
