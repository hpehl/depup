# CI/CD Auto-PR Design

## Goal

Enhance the depup GitHub Action to automatically create pull requests for outdated dependencies that GitHub's dependabot cannot handle. This complements dependabot rather than replacing it.

## What Dependabot Can't Handle (depup's value-add)

1. **Maven property-based versions** — `<properties>` entries like `version.junit` that drive multiple dependencies
2. **Tool version properties** — Node.js, npm, pnpm, yarn versions managed in Maven POMs
3. **Custom Maven repositories** — artifacts not on Maven Central
4. **npm `packageManager` field** — the `"packageManager": "pnpm@9.15.0"` field in `package.json`

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Outcome | Auto-create PRs | Fills dependabot's gap directly |
| PR grouping | One PR per category | Balance between isolation and PR spam |
| Configuration | Action inputs only | No config file (YAGNI) |
| Existing PRs | Skip if open PR exists | Simplest, matches dependabot behavior |
| Orchestration | Action-level via `gh` CLI | Keeps depup a pure CLI tool |
| Filter approach | Action owns category flags, user controls stable/include/exclude | Clean separation, no flag conflicts |

## Categories

The action loops over 6 categories, each producing at most one PR:

| Category | depup flags | Branch name |
|----------|-----------|-------------|
| Maven managed deps | `--maven --dependencies --managed` | `depup/maven-managed-dependencies` |
| Maven unmanaged deps | `--maven --dependencies --unmanaged` | `depup/maven-unmanaged-dependencies` |
| Maven managed plugins | `--maven --plugins --managed` | `depup/maven-managed-plugins` |
| Maven unmanaged plugins | `--maven --plugins --unmanaged` | `depup/maven-unmanaged-plugins` |
| Maven tools | `--maven --tools` | `depup/maven-tools` |
| npm tools | `--npm --tools` | `depup/npm-tools` |

## Action Inputs

```yaml
inputs:
  path:
    description: 'Path to the project root'
    required: false
    default: '.'
  version:
    description: 'depup version to install'
    required: false
    default: 'latest'
  stable:
    description: 'Exclude pre-release versions (alpha, beta, RC, milestone)'
    required: false
    default: 'false'
  include:
    description: 'Only include Maven artifacts matching glob patterns (comma-separated). Exclude takes precedence over include.'
    required: false
    default: ''
  exclude:
    description: 'Exclude Maven artifacts matching glob patterns (comma-separated). Takes precedence over include.'
    required: false
    default: ''
  token:
    description: 'GitHub token for creating PRs and branches'
    required: false
    default: ${{ github.token }}
  base-branch:
    description: 'Branch to create PRs against (defaults to repository default branch)'
    required: false
    default: ''
  labels:
    description: 'Comma-separated PR labels (must already exist in the repo)'
    required: false
    default: 'dependencies'
```

## Action Outputs

| Output | Description |
|--------|-------------|
| `exit-code` | 0=no outdated deps, 1=outdated deps found |
| `json` | Path to raw JSON output from the check phase |

## PR Creation Flow

For each category:

1. **Build flags** — combine category-specific flags (`--maven --dependencies --managed`) with user flags (`--stable`, `--include`, `--exclude`)
2. **Check** — run `depup check --json --outdated <flags>` and capture JSON output
3. **Skip if empty** — if no outdated dependencies in this category, move to next
4. **Skip if PR exists** — check `gh pr list --head <branch> --state open`, skip if found
5. **Create branch** — `git checkout -b <branch>` from base branch
6. **Update** — run `depup update <flags>` (same flags as step 2, minus `--json --outdated`) to edit files in the working tree
7. **Check for changes** — `git diff --quiet`, skip if no changes
8. **Commit** — `git add -A && git commit -m "<title>"`
9. **Push** — `git push -u origin <branch>`
10. **Create PR** — `gh pr create --base <base> --head <branch> --title "<title>" --body "<body>" --label "<labels>"`
11. **Reset** — `git checkout <base-branch> && git branch -D <branch>`

## PR Identity

PRs are created by whoever owns the `token` input — by default `github-actions[bot]`. depup PRs are identifiable by:

1. **Branch prefix** — all branches start with `depup/` (e.g., `depup/maven-managed-dependencies → main`)
2. **Labels** — the `labels` input (default: `dependencies`) is applied to all PRs; labels must already exist in the repo
3. **PR body footer** — "This PR was automatically created by depup"

## PR Format

**Title:** `chore(deps): bump <category-label>`

Examples:
- `chore(deps): bump Maven managed dependencies`
- `chore(deps): bump Maven plugins`
- `chore(deps): bump npm packageManager versions`

**Body:**

```markdown
Bumps outdated <category-label>.

| Artifact | Current | Latest |
|----------|---------|--------|
| `org.junit.jupiter:junit-jupiter` | 5.10.0 | 5.12.0 |
| `org.junit.platform:junit-platform-launcher` | 1.10.0 | 1.12.0 |

---
This PR was automatically created by [depup](https://github.com/hpehl/depup).
```

## Usage Examples

### Minimal — check everything, create PRs for all categories

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

## Prerequisites (completed)

### Filter Logic Enhancement

The CLI filter was updated to allow multiple `--kind` flags combined with OR semantics:

- `Filter.kind: Option<KindFilter>` changed to `Filter.kinds: Vec<KindFilter>`
- Kind flags (`--dependencies`, `--plugins`, `--dev-deps`, `--tools`) are no longer mutually exclusive
- Multiple kind flags combine with OR; all other filters combine with AND
- Ecosystem flags (`--maven`/`--npm`) remain mutually exclusive

This enables the action to run focused checks like `--maven --dependencies --managed` without the kind filter conflicting with other flags.

## Scope Exclusions

- **npm deps/devDeps** — dependabot handles these well; excluded from auto-PRs
- **Dependabot replacement** — this complements dependabot, not replaces it
- **Config file** — no `.depup.yml`; everything via action inputs
- **Cross-product filters** — "all Maven + only npm tools" in a single CLI invocation is not supported; the action handles this by running separate checks per category
