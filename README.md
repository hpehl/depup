# depup

Check, update, and audit dependency versions across multiple ecosystems.

`depup` auto-detects project ecosystems in a directory tree and checks all dependencies for newer versions and known vulnerabilities. It supports **Maven** and **npm** (with npm, pnpm, yarn classic, and bun package managers).

**[Full Documentation](https://hpehl.github.io/depup)**

## Features

- **Check** — compare installed versions against the latest upstream releases
- **Update** — rewrite version numbers in place (Maven POMs) or delegate to the native package manager (npm)
- **Audit** — query [OSV.dev](https://osv.dev/) for known vulnerabilities
- **GitHub Action** — automatically create PRs for outdated dependencies that Dependabot cannot handle

## Installation

### Homebrew

```bash
brew tap hpehl/tap
brew install depup
```

### Cargo

```bash
cargo install depup-cli
```

### Precompiled Binaries

Download from [GitHub Releases](https://github.com/hpehl/depup/releases) for macOS (Intel & Apple Silicon), Linux, and Windows.

## Quick Start

```bash
# Check for outdated dependencies
depup check

# Update all outdated dependencies
depup update

# Audit for known vulnerabilities
depup audit
```

## GitHub Action

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
      - uses: actions/checkout@v7
      - uses: hpehl/depup@v2
```

For npm projects, the package manager must be installed on the runner before the depup action. For example, with pnpm:

```yaml
steps:
  - uses: actions/checkout@v7
  - uses: pnpm/action-setup@v6
  - uses: actions/setup-node@v6
    with:
      node-version: 'lts/*'
  - uses: hpehl/depup@v2
```

See the [GitHub Action documentation](https://hpehl.github.io/depup/github-action/overview.html) for setup, inputs, and examples.

## Documentation

Full documentation is available at **[hpehl.github.io/depup](https://hpehl.github.io/depup)**, covering:

- [Installation options](https://hpehl.github.io/depup/installation.html)
- [Usage (check, update, audit, completions)](https://hpehl.github.io/depup/usage/check.html)
- [Ecosystem details (Maven, npm)](https://hpehl.github.io/depup/ecosystems/maven.html)
- [Filtering and reference](https://hpehl.github.io/depup/reference/filtering.html)
- [GitHub Action setup and examples](https://hpehl.github.io/depup/github-action/overview.html)

## License

Apache License 2.0
