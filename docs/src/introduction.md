# Introduction

`depup` is a CLI tool that checks dependency versions across multiple ecosystems, updates outdated dependencies in place, and audits them for known vulnerabilities.

Point it at any directory tree and it auto-detects project ecosystems, discovers all dependencies, and tells you what's outdated or vulnerable.

## Features

- **Multi-ecosystem** — supports Maven and npm (with npm, pnpm, yarn classic, and bun package managers)
- **Auto-detection** — discovers ecosystems from project files (`pom.xml`, lock files, `packageManager` field)
- **Check** — compare installed versions against the latest upstream releases
- **Update** — rewrite version numbers in place (Maven POMs) or delegate to the native package manager (npm)
- **Audit** — query [OSV.dev](https://osv.dev/) for known vulnerabilities in all dependencies
- **Filtering** — narrow results by ecosystem, kind, version management style, or artifact name globs
- **JSON output** — machine-readable output for scripting and CI/CD pipelines
- **Shell completions** — tab-completion for bash, zsh, fish, elvish, and powershell
- **GitHub Action** — automatically create PRs for outdated dependencies that Dependabot cannot handle
- **Granular exit codes** — different codes for outdated deps vs. vulnerabilities for CI integration

## Quick Start

```bash
# Install via Homebrew
brew tap hpehl/tap
brew install depup

# Check current directory for outdated dependencies
depup check

# Update all outdated dependencies
depup update

# Audit for known vulnerabilities
depup audit
```

For installation options, see [Installation](installation.md). For detailed usage, see the [Check](usage/check.md), [Update](usage/update.md), and [Audit](usage/audit.md) pages.
