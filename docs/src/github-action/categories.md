# Categories

The action processes 8 dependency categories, creating one PR per category when outdated dependencies are found. This separation keeps PRs focused and makes review easier.

## Category Table

| Category | depup Flags | Branch Name |
|----------|-------------|-------------|
| Maven managed dependencies | `--maven --dependencies --managed` | `depup/maven-managed-dependencies` |
| Maven unmanaged dependencies | `--maven --dependencies --unmanaged` | `depup/maven-unmanaged-dependencies` |
| Maven managed plugins | `--maven --plugins --managed` | `depup/maven-managed-plugins` |
| Maven unmanaged plugins | `--maven --plugins --unmanaged` | `depup/maven-unmanaged-plugins` |
| Maven tool versions | `--maven --tools` | `depup/maven-tools` |
| npm packageManager versions | `--npm --tools` | `depup/npm-tools` |
| npm dependencies | `--npm --dependencies` | `depup/npm-dependencies` |
| npm dev dependencies | `--npm --dev-deps` | `depup/npm-dev-dependencies` |

> **Note:** npm dependency and dev dependency categories require the package manager to be installed on the runner. See [Setup — npm Projects](setup.md#npm-projects).

## Why Separate Categories?

- **Managed vs. unmanaged** — Maven properties affect multiple dependencies across modules. Grouping property-based updates separately from inline version updates helps reviewers understand the impact.
- **Dependencies vs. plugins** — Plugin updates can affect the build process itself and deserve separate review.
- **Tools** — Node.js and package manager version updates are infrastructure changes, separate from library updates.
- **npm dependencies vs. dev dependencies** — Production dependencies and dev-only dependencies have different risk profiles and review needs.
- **npm tools** — Package manager version updates in `package.json` are infrastructure changes, separate from package updates.

## Branch Naming

All branches use the `depup/` prefix, making them easy to identify. The suffix matches the category name in kebab-case.

## PR Format

### Title

```
chore(deps): bump <category-label>
```

Examples:
- `chore(deps): bump Maven managed dependencies`
- `chore(deps): bump npm packageManager versions`

### Body

The PR body contains a Markdown table listing each updated artifact:

```markdown
## Outdated <category-label>

| Artifact | Current | Latest |
|----------|---------|--------|
| org.junit.jupiter:junit-jupiter | 5.10.0 | 5.11.3 |
| org.mockito:mockito-core | 5.11.0 | 5.14.2 |

---
*Created by [depup](https://github.com/hpehl/depup)*
```

## Skipping

A category is skipped when:

- **No outdated dependencies** — `depup check` finds nothing outdated in this category
- **PR already exists** — an open PR already exists on the category's branch
- **No file changes** — `depup update` ran but produced no actual file changes
- **Update failed** — `depup update` returned an error (a warning is logged)
