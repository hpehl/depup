# depup

Check dependency versions across multiple ecosystems.

`depup` auto-detects project ecosystems in a directory tree and checks all dependencies for newer versions. Currently supports **Maven** and **pnpm**.

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

If both Maven and pnpm projects are found in the target path, both are checked and results are combined.

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

Also detects Node.js and npm/pnpm/yarn version properties in Maven POMs (e.g., `version.node`, `version.npm`).

### pnpm

Discovers pnpm projects in the directory tree (via `pnpm-lock.yaml` or `packageManager` field in `package.json`), runs `pnpm outdated --format json` on each, and aggregates results. Workspace members are skipped — only root projects are checked.

## Example Output

```
🔍 Discovering POM modules...
⚙️ Checking 4 properties...
  ✓ version.compiler.plugin  org.apache.maven.plugins:maven-compiler-plugin  3.13.0    up-to-date
  → version.junit            org.junit.jupiter:junit-jupiter                 5.10.0    → 5.12.2
  ✓ version.javadoc.plugin   org.apache.maven.plugins:maven-javadoc-plugin   3.12.0    up-to-date
  ✓ version.mockito          org.mockito:mockito-core                        5.18.0    up-to-date

4 properties checked: 3 current, 1 outdated  (■ Dependency, ■ Plugin)

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

### pnpm

1. Walks the directory tree finding directories with `pnpm-lock.yaml` or `packageManager: "pnpm@..."` in `package.json`
2. Skips `node_modules/` and workspace members
3. Runs `pnpm outdated --format json` on each discovered project
4. Parses and aggregates results

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
- `pnpm` installed and on PATH (for pnpm ecosystem)

## License

Apache License 2.0
