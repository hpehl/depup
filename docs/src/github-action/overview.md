# GitHub Action Overview

`depup` includes a composite GitHub Action that automatically creates pull requests for outdated dependencies. It complements GitHub's Dependabot by handling dependency types that Dependabot cannot manage.

## Why This Action?

Dependabot is great for standard `package.json` dependencies and Maven dependencies with inline versions. However, it cannot handle:

- **Maven property-based versions** — `<properties>` entries like `version.junit` that drive multiple dependencies across modules
- **Tool version properties** — Node.js, npm, pnpm, yarn versions managed in Maven POMs (e.g., `version.node`)
- **Custom Maven repositories** — artifacts hosted on private or non-Central repositories
- **npm `packageManager` field** — the `"packageManager": "pnpm@9.15.0"` field in `package.json`

The `depup` action fills these gaps by running the same check and update pipeline you use locally, then creating PRs for each dependency category.

## How It Works

The action loops over 6 dependency categories. For each category:

1. **Check** — runs `depup check --json --outdated` with category-specific flags
2. **Skip if empty** — moves to the next category if no outdated dependencies
3. **Skip if PR exists** — checks for open PRs on the category's branch
4. **Create branch** — `git checkout -b depup/<category>` from the base branch
5. **Update** — runs `depup update` to modify files in the working tree
6. **Commit & push** — commits all changes and pushes to origin
7. **Create PR** — creates a pull request via `gh pr create` with a descriptive title and body
8. **Reset** — returns to the base branch

PR titles follow the format `chore(deps): bump <category-label>`. PR bodies contain a table listing each artifact with its current and latest version.

For details on the 6 categories, see [Categories](categories.md).

## Quick Start

```yaml
name: depup
on:
  schedule:
    - cron: '0 6 * * 1'
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

For complete setup instructions, see [Setup](setup.md).
