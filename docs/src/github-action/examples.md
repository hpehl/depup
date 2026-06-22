# Examples

## Minimal — Check Everything Weekly

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

## Stable Versions Only, Exclude Test Libraries

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
        with:
          stable: true
          exclude: 'org.junit:*,org.mockito:*'
```

## Only Specific Artifacts

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
        with:
          include: 'org.wildfly:*,org.jboss:*'
```

## Custom Labels and Base Branch

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
        with:
          base-branch: 'develop'
          labels: 'dependencies,automated'
```

## pnpm Project

For npm ecosystem projects, the package manager must be installed on the runner before the depup action:

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
      - uses: pnpm/action-setup@v4
        with:
          version: 10
      - uses: actions/setup-node@v4
        with:
          node-version: '22'
      - uses: hpehl/depup@v1
```

## Monorepo — Scan a Subdirectory

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
        with:
          path: 'services/backend'
```

## Using a PAT for CI Trigger

If you need PRs created by the action to trigger other workflows (e.g., CI checks), use a Personal Access Token instead of the default `GITHUB_TOKEN`:

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
        with:
          token: ${{ secrets.DEPUP_PAT }}
```

## Daily Check with Output Handling

```yaml
name: depup
on:
  schedule:
    - cron: '0 6 * * *'
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
        id: depup
        with:
          stable: true

      - name: Summary
        run: |
          if [ "${{ steps.depup.outputs.exit-code }}" = "1" ]; then
            echo "::notice::depup created PRs for outdated dependencies"
          else
            echo "::notice::All dependencies are up to date"
          fi
```
