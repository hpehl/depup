# Audit

The `audit` subcommand queries [OSV.dev](https://osv.dev/) for known vulnerabilities in all discovered dependencies. It works for both Maven and npm ecosystems using the same unified API.

```bash
depup audit [OPTIONS] [PATH]
```

If no path is given, the current directory is used.

## Basic Usage

```bash
# Audit all dependencies for known vulnerabilities
depup audit

# Audit a specific project
depup audit /path/to/project
```

## Severity Filtering

```bash
# Only critical vulnerabilities
depup audit --severity critical

# Critical + high
depup audit --severity high
```

The severity levels, from most to least severe, are: **critical**, **high**, **medium**, **low**.

When you specify a severity level, all vulnerabilities at that level and above are included. For example, `--severity high` includes both critical and high severity vulnerabilities.

## Dependency Filtering

```bash
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

# Filter by artifact name
depup audit --include 'org.wildfly:*'
depup audit --exclude '*:guava'
```

> **Note:** Tool versions (Node.js, package manager versions) are excluded from audit results — they aren't registry packages with OSV vulnerability advisories. The `--tools` filter is not available for the audit subcommand.

For a comprehensive guide to all filter options, see [Filtering](../reference/filtering.md).

## How It Works

1. Reuses the check pipeline to discover all dependencies with their current versions
2. Filters out tool versions (not applicable for vulnerability checks)
3. Sends a batch query to the [OSV.dev API](https://osv.dev/) (`POST /v1/querybatch`) with dependency coordinates and versions
4. Fetches full vulnerability details for each match (`GET /v1/vulns/{id}`)
5. Extracts severity from CVSS scores or ecosystem/database-specific labels
6. Deduplicates vulnerability IDs across dependencies
7. Reports results grouped by ecosystem and kind

## JSON Output

```bash
depup audit --json
```

See [JSON Mode](../reference/json-mode.md) for details.

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | No vulnerabilities found |
| 2 | Vulnerabilities found (any severity) |
| 3 | Critical or high severity vulnerabilities found |

See [Exit Codes](../reference/exit-codes.md) for the full table and CI integration examples.
