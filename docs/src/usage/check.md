# Check

The `check` subcommand compares installed dependency versions against the latest upstream releases and reports what's outdated.

```bash
depup check [OPTIONS] [PATH]
```

If no path is given, the current directory is used. If both Maven and npm ecosystem projects are found, both are processed and results are combined.

## Basic Usage

```bash
# Check current directory (auto-detects ecosystems)
depup check

# Check a specific project
depup check /path/to/project

# Only show outdated versions
depup check --outdated
```

## Filtering

```bash
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
depup check --include 'org.junit:*'
depup check --exclude '*:guava'
depup check --include 'org.wildfly:*' --exclude '*:core'
```

For a comprehensive guide to all filter options, see [Filtering](../reference/filtering.md).

## JSON Output

```bash
depup check --json
```

Progress bars are suppressed in JSON mode, and the output is a JSON array of dependency results. See [JSON Mode](../reference/json-mode.md) for details on the output format.

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | All dependencies are up to date |
| 1 | Outdated dependencies found |

See [Exit Codes](../reference/exit-codes.md) for the full table across all subcommands.
