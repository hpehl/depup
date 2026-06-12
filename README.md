# mvnup

Check Maven version properties against upstream registries.

`mvnup` scans a multi-module Maven project, discovers all `${version.*}` properties and the artifacts they control, then checks each against upstream Maven repositories to find outdated versions. It works where Maven's `versions:display-property-updates` fails — when properties are defined in a parent POM but referenced in child POMs.

## Installation

```bash
cargo install --path .
```

## Usage

```bash
# Check current directory
mvnup

# Check a specific project
mvnup check /path/to/maven/project

# JSON output (for scripting)
mvnup check --json

# Only show outdated versions
mvnup check --outdated

# Include alpha, beta, RC, milestone versions
mvnup check --include-pre-releases

# Verbose output (show artifact coordinates)
mvnup check -v

# Generate shell completions (auto-detects shell)
mvnup completions

# Install shell completions
mvnup completions --install

# Generate completions for a specific shell
mvnup completions fish
```

The `check` subcommand is the default — `mvnup /path` is equivalent to `mvnup check /path`.

## Example Output

```
🔍 Discovering POM modules...
🌐 Checking 4 properties...
  ✓ version.compiler.plugin
  ✓ version.javadoc.plugin
  ✓ version.junit
  ✓ version.mockito

+----------------------------+---------+--------+----------+
| Property                   | Current | Latest | Status   |
+============================================================+
| version.compiler.plugin    | 3.11.0  | 3.13.0 | OUTDATED |
|----------------------------+---------+--------+----------|
| version.javadoc.plugin     | 3.12.0  | 3.12.0 | OK       |
|----------------------------+---------+--------+----------|
| version.junit              | 5.10.0  | 5.12.2 | OUTDATED |
|----------------------------+---------+--------+----------|
| version.mockito            | 5.18.0  | 5.18.0 | OK       |
+----------------------------+---------+--------+----------+

4 properties checked: 2 current, 2 outdated

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

1. Parses the root `pom.xml` and recursively follows `<modules>` declarations
2. For every `<dependency>` and `<plugin>` using `${version.*}`, maps the property to its groupId and artifactId
3. Resolves property values from the root POM's `<properties>` block
4. Queries Maven Central for the latest version of each artifact (via `maven-metadata.xml`)
5. If not found on Maven Central, queries all `<repositories>` and `<pluginRepositories>` defined in the POMs in parallel
6. Compares versions using Maven-aware ordering (handles `.Final`, `-SP1`, and other qualifiers)

## Version Filtering

By default, `mvnup` only considers stable releases. Pre-release versions matching these patterns are excluded:

- `*-SNAPSHOT`
- `*-alpha*`, `*-beta*`
- `*-RC*`, `*-CR*`
- `*-M*` (milestones)
- `*-preview*`, `*-dev*`, `*-incubating*`

Use `--include-pre-releases` to include them (SNAPSHOTs are always excluded).

## Shell Completions

Generate and install shell completions for tab-completion of subcommands and flags:

```bash
mvnup completions --install       # auto-detect shell, install to standard path
mvnup completions fish            # print fish completions to stdout
mvnup completions --install zsh   # install zsh completions
```

Supported shells: bash, zsh, fish, elvish, powershell.

## Requirements

- Rust 1.85+ (edition 2024)
- Network access to Maven Central (`repo1.maven.org`) and any custom repositories defined in the project's POMs

## License

Apache License 2.0
