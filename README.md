# mvnup

Check Maven version properties against upstream registries.

`mvnup` scans a multi-module Maven project, discovers all `${version.*}` properties and the artifacts they control, then checks each against Maven Central to find outdated versions. It works where Maven's `versions:display-property-updates` fails — when properties are defined in a parent POM but referenced in child POMs.

## Installation

```bash
cargo install --path .
```

## Usage

```bash
# Check current directory
mvnup

# Check a specific project
mvnup /path/to/maven/project

# JSON output (for scripting)
mvnup --json

# Only show outdated versions
mvnup --outdated

# Include alpha, beta, RC, milestone versions
mvnup --include-pre-releases

# Verbose output (show artifact coordinates)
mvnup -v
```

## Example Output

```
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
```

The exit code is `0` when all versions are current, `1` when any are outdated.

## How It Works

1. Parses the root `pom.xml` and recursively follows `<modules>` declarations
2. For every `<dependency>` and `<plugin>` using `${version.*}`, maps the property to its groupId and artifactId
3. Resolves property values from the root POM's `<properties>` block
4. Queries Maven Central for the latest version of each artifact
5. Compares versions using Maven-aware ordering (handles `.Final`, `-SP1`, and other qualifiers)

## Version Filtering

By default, `mvnup` only considers stable releases. Pre-release versions matching these patterns are excluded:

- `*-SNAPSHOT`
- `*-alpha*`, `*-beta*`
- `*-RC*`, `*-CR*`
- `*-M*` (milestones)
- `*-preview*`, `*-dev*`, `*-incubating*`

Use `--include-pre-releases` to include them (SNAPSHOTs are always excluded).

## Requirements

- Rust 1.85+ (edition 2024)
- Network access to Maven Central (`search.maven.org`)

## License

Apache License 2.0
