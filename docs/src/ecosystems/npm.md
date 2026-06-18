# npm

`depup` discovers npm ecosystem projects in a directory tree and checks for outdated packages. It supports multiple package managers and auto-detects which one each project uses.

## Supported Package Managers

| Package Manager | Lock File | Check Command | Update Command |
|----------------|-----------|---------------|----------------|
| npm | `package-lock.json` | `npm list` + `npm outdated` | `npm update` |
| pnpm | `pnpm-lock.yaml` | `pnpm list` + `pnpm outdated` | `pnpm update` |
| yarn (classic) | `yarn.lock` | `yarn list` + `yarn outdated` | `yarn upgrade` |
| bun | `bun.lock` / `bun.lockb` | `package.json` + `bun outdated` | `bun update` |

## Auto-Detection

The package manager is detected by examining the project directory for:

1. **Lock files** — the presence of a specific lock file determines the package manager
2. **`packageManager` field** — the `"packageManager"` field in `package.json` (e.g., `"packageManager": "pnpm@9.15.0"`)

Lock file detection takes precedence. If no lock file is found, the `packageManager` field is used.

## Project Discovery

`depup` walks the directory tree and finds all directories that contain a recognized lock file or a `package.json` with a `packageManager` field. The following directories are automatically skipped:

- `node_modules`
- `.pnpm-store`
- `.yarn`
- `.bun`
- Other build and cache directories

### Workspace Handling

Workspace members are skipped — only root projects are checked. Workspace detection works per package manager:

- **pnpm** — workspace members defined in `pnpm-workspace.yaml`
- **npm / yarn / bun** — workspace members defined in the `workspaces` field of `package.json`

## Package Manager Versions

The `packageManager` field in `package.json` (e.g., `"packageManager": "pnpm@9.15.0"`) is also checked for updates. `depup` queries the npm registry for the latest version of the package manager itself.

When updating, the `packageManager` field is rewritten in `package.json`. Corepack `+hash` suffixes are stripped during version comparison.

## pnpm Catalogs

[pnpm catalogs](https://pnpm.io/catalogs) (`"catalog:<name>"` version specifiers defined in `pnpm-workspace.yaml`) are resolved transparently by pnpm's own commands. `depup` does not need to handle them explicitly — versions are resolved correctly via `pnpm list`/`pnpm outdated`, and updates are handled by `pnpm update`.

## Requirements

The respective package manager must be installed and available on `PATH`:

- npm ecosystem checks require `npm`
- pnpm ecosystem checks require `pnpm`
- yarn ecosystem checks require `yarn`
- bun ecosystem checks require `bun`
