# Version Filtering

`depup` provides options to control which upstream versions are considered when checking for updates.

## Stable Versions Only

Use `--stable` (alias `--releases-only`) to exclude pre-release versions:

```bash
depup check --stable
depup update --stable
```

Pre-release versions matching these patterns are excluded:

| Pattern | Example |
|---------|---------|
| `*-alpha*` | `2.0.0-alpha1` |
| `*-beta*` | `3.1.0-beta2` |
| `*-RC*` | `4.0.0-RC1` |
| `*-CR*` | `1.5.0-CR1` |
| `*-M*` | `2.0.0-M3` (milestones) |
| `*-preview*` | `5.0.0-preview.1` |
| `*-dev*` | `1.0.0-dev.5` |
| `*-incubating*` | `3.0.0-incubating` |

## SNAPSHOT Exclusion

Maven SNAPSHOT versions are **always** excluded, regardless of flags. You never need to explicitly filter them out.

## Default Behavior

By default (without `--stable`), `depup` includes pre-release versions but always excludes SNAPSHOTs. This means you'll see alpha, beta, and RC versions in the results unless you opt out with `--stable`.
