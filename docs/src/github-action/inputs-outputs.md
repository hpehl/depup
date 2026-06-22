# Inputs & Outputs

## Inputs

All inputs are optional.

| Input | Default | Description |
|-------|---------|-------------|
| `path` | `.` | Path to the project root |
| `version` | `latest` | depup version to install (e.g., `0.4.0`) |
| `stable` | `false` | Exclude pre-release versions (alpha, beta, RC, milestone) |
| `include` | | Only include artifacts matching glob patterns (comma-separated). Exclude takes precedence over include. |
| `exclude` | | Exclude artifacts matching glob patterns (comma-separated). Takes precedence over include. |
| `token` | `github.token` | GitHub token for creating PRs and branches |
| `base-branch` | | Branch to create PRs against (defaults to repository default branch) |
| `labels` | `dependencies` | Comma-separated PR labels (must already exist in the repo) |

### `path`

The directory to scan for projects. Relative to the repository root.

```yaml
- uses: hpehl/depup@v2
  with:
    path: 'backend'
```

### `version`

Pin the depup version instead of using the latest release. Useful for reproducibility.

```yaml
- uses: hpehl/depup@v2
  with:
    version: '0.4.0'
```

The version can be specified with or without the `v` prefix — both `0.4.0` and `v0.4.0` work.

### `stable`

When set to `true`, excludes pre-release versions (alpha, beta, RC, milestones) from consideration. See [Version Filtering](../reference/version-filtering.md) for details on which patterns are excluded.

```yaml
- uses: hpehl/depup@v2
  with:
    stable: true
```

### `include` and `exclude`

Glob patterns to filter which artifacts are checked and updated. Patterns use `*` wildcards and are comma-separated.

```yaml
- uses: hpehl/depup@v2
  with:
    include: 'org.wildfly:*,org.jboss:*'
    exclude: '*:test-utils'
```

When both are specified, `exclude` takes precedence. See [Filtering](../reference/filtering.md) for details.

### `token`

GitHub token used for creating branches, pushing commits, and creating PRs. Defaults to the workflow's `GITHUB_TOKEN`.

```yaml
- uses: hpehl/depup@v2
  with:
    token: ${{ secrets.MY_PAT }}
```

### `base-branch`

The branch to create PRs against. If not specified, the repository's default branch is used.

```yaml
- uses: hpehl/depup@v2
  with:
    base-branch: 'develop'
```

### `labels`

Comma-separated list of labels to apply to created PRs. Labels must already exist in the repository.

```yaml
- uses: hpehl/depup@v2
  with:
    labels: 'dependencies,automated,bot'
```

## Outputs

| Output | Description |
|--------|-------------|
| `exit-code` | `0` if no outdated dependencies found, `1` if outdated dependencies were found |

### Using the Output

```yaml
- uses: hpehl/depup@v2
  id: depup

- name: Check result
  run: |
    if [ "${{ steps.depup.outputs.exit-code }}" = "1" ]; then
      echo "Outdated dependencies were found and PRs were created"
    fi
```
