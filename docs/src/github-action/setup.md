# Setup

This guide walks through setting up the `depup` GitHub Action in your repository.

## Prerequisites

- A GitHub repository with Maven or npm projects
- GitHub Actions enabled on the repository

## Minimal Workflow

Create `.github/workflows/depup.yml`:

```yaml
name: depup
on:
  schedule:
    - cron: '0 6 * * 1'  # Weekly on Monday at 6am UTC
  workflow_dispatch:       # Allow manual trigger

permissions:
  contents: write          # Required for creating branches and pushing
  pull-requests: write     # Required for creating PRs

jobs:
  update:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: hpehl/depup@v1
```

This will:
- Run every Monday at 6am UTC
- Check all ecosystems for outdated dependencies
- Create one PR per dependency category
- Skip categories with no outdated dependencies or existing open PRs

## Permissions

The action requires two permissions:

| Permission | Why |
|------------|-----|
| `contents: write` | Creating branches and pushing commits |
| `pull-requests: write` | Creating pull requests |

These are set at the workflow level via the `permissions` block.

## Token

By default, the action uses the built-in `github.token` (aka `GITHUB_TOKEN`). This works for most cases. If you need PRs to trigger other workflows (e.g., CI checks), you'll need to use a [Personal Access Token (PAT)](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/creating-a-personal-access-token) or a [GitHub App token](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/about-authentication-with-a-github-app):

```yaml
- uses: hpehl/depup@v1
  with:
    token: ${{ secrets.MY_PAT }}
```

> **Note:** The default `GITHUB_TOKEN` does not trigger other workflows when it creates PRs. This is a GitHub security feature. Use a PAT or App token if your PRs need to trigger CI.

## Labels

By default, PRs are labeled with `dependencies`. You can customize this:

```yaml
- uses: hpehl/depup@v1
  with:
    labels: 'dependencies,automated'
```

> **Important:** Labels must already exist in the repository. The action does not create labels.

## Base Branch

By default, PRs target the repository's default branch. To target a different branch:

```yaml
- uses: hpehl/depup@v1
  with:
    base-branch: 'develop'
```

## Specifying a depup Version

By default, the action installs the latest release. To pin a specific version:

```yaml
- uses: hpehl/depup@v1
  with:
    version: '0.4.0'
```

## Scheduling

The `schedule` trigger uses [cron syntax](https://docs.github.com/en/actions/using-workflows/events-that-trigger-workflows#schedule). Common schedules:

| Schedule | Cron |
|----------|------|
| Weekly on Monday at 6am | `0 6 * * 1` |
| Daily at midnight | `0 0 * * *` |
| Twice a week (Mon + Thu) | `0 6 * * 1,4` |
| Monthly on the 1st | `0 6 1 * *` |

Adding `workflow_dispatch` allows you to trigger the action manually from the GitHub UI at any time.
