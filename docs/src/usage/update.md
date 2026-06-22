# Update

The `update` subcommand updates outdated dependencies in place. It reuses the check pipeline to identify what's outdated, then applies updates.

```bash
depup update [OPTIONS] [PATH]
```

If no path is given, the current directory is used.

## Basic Usage

```bash
# Update all outdated dependencies
depup update

# Update a specific project
depup update /path/to/project

# Preview what would be updated (no changes made)
depup update --dry-run
```

## Filtering

All check filters are available for update as well:

```bash
# Only update to stable releases
depup update --stable

# Filter by ecosystem
depup update --maven
depup update --npm

# Filter by kind
depup update --dependencies
depup update --plugins
depup update --dev-dependencies
depup update --tools

# Filter by version property (Maven only)
depup update --managed
depup update --unmanaged

# Filter by artifact name
depup update --include 'org.junit:*'
depup update --exclude '*:guava'
depup update --include 'react*'
```

For a comprehensive guide to all filter options, see [Filtering](../reference/filtering.md).

## How Updates Work

### Maven

`depup update` rewrites version numbers in POM files while preserving all formatting, comments, and indentation. Both types of version references are updated:

- **Managed properties** — `${...}` references in `<properties>` blocks (e.g., `<version.junit>5.10.0</version.junit>`)
- **Plain inline versions** — hardcoded version numbers inside dependency/plugin blocks (e.g., `<version>5.10.0</version>`)

The updater processes each POM file that contains outdated versions, applying property updates first, then inline version updates.

### npm

For npm ecosystem projects, `depup update` delegates to the detected package manager's native update command:

| Package Manager | Update Command |
|----------------|----------------|
| npm | `npm update` |
| pnpm | `pnpm update` |
| yarn (classic) | `yarn upgrade` |
| bun | `bun update` |

## JSON Output

```bash
depup update --json
```

See [JSON Mode](../reference/json-mode.md) for details.

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | All updates succeeded (or no outdated dependencies found) |
| 1 | One or more updates failed |

See [Exit Codes](../reference/exit-codes.md) for the full table.
