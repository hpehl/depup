# Filtering

`depup` provides composable filters to narrow results by ecosystem, dependency kind, version management style, and artifact name. All filters work across `check`, `update`, and `audit` subcommands (with minor exceptions noted below).

## Ecosystem Filters

Restrict results to a single ecosystem:

```bash
depup check --maven        # only Maven dependencies
depup check --npm          # only npm dependencies
```

If neither is specified, all detected ecosystems are included.

## Kind Filters

Filter by the type of dependency:

| Flag | Description | Ecosystems |
|------|-------------|------------|
| `--dependencies` | Regular dependencies | Maven, npm |
| `--plugins` | Maven plugins | Maven |
| `--dev-dependencies` | Development dependencies | npm |
| `--tools` | Tool versions (Node.js, package managers) | Maven, npm |

Multiple kind filters can be combined — they act as a union (any match is included).

> **Note:** The `--tools` filter is not available for the `audit` subcommand. Tool versions (Node.js, package manager versions) are excluded from audit because they aren't registry packages with OSV vulnerability advisories.

## Version Property Filters (Maven Only)

Filter by how the version is managed in the POM:

```bash
depup check --managed      # only dependencies using a ${...} property
depup check --unmanaged    # only dependencies with plain inline versions
```

- **Managed** — the version is defined via a `<properties>` entry (e.g., `${junit.version}`)
- **Unmanaged** — the version is hardcoded inline in the dependency/plugin block

## Artifact Name Filters

Filter dependencies by name using glob patterns with `*` wildcards:

```bash
# Include only matching artifacts
depup check --include 'org.junit:*'

# Exclude matching artifacts
depup check --exclude '*:guava'

# Combine include and exclude (exclude takes precedence)
depup check --include 'org.wildfly:*' --exclude '*:core'
```

For Maven, the pattern matches against `groupId:artifactId` (e.g., `org.junit.jupiter:junit-jupiter`).

For npm, the pattern matches against the package name (e.g., `react`, `@types/node`).

Multiple `--include` and `--exclude` flags can be specified. Only `*` wildcards are supported (no regex, no `?`).

### Precedence

When both `--include` and `--exclude` are specified, `--exclude` takes precedence. A dependency must match at least one include pattern AND not match any exclude pattern to be included.

## Outdated Filter

Show only dependencies where a newer version is available:

```bash
depup check --outdated
```

This is particularly useful for focusing on actionable results.

## Severity Filter (Audit Only)

Filter audit results by minimum vulnerability severity:

```bash
depup audit --severity critical    # only critical
depup audit --severity high        # critical + high
depup audit --severity medium      # critical + high + medium
depup audit --severity low         # all severities (default)
```

## Vulnerable Filter (Audit Only)

Show only dependencies that have known vulnerabilities:

```bash
depup audit --vulnerable
```

## Combining Filters

All filters are composable. For example:

```bash
# Only outdated Maven dependencies (not plugins), excluding test libs
depup check --outdated --maven --dependencies --exclude 'org.junit:*,org.mockito:*'

# Only managed Maven plugins with stable versions
depup check --maven --plugins --managed --stable

# Audit npm dependencies for high+ severity vulnerabilities
depup audit --npm --dependencies --severity high
```
