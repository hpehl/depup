# Installation

[Precompiled binaries](https://github.com/hpehl/depup/releases) are available for macOS (Intel & Apple Silicon), Linux, and Windows.

## Homebrew

```bash
brew tap hpehl/tap
brew install depup
```

## Cargo

```bash
cargo install depup-cli
```

The crate is published to [crates.io](https://crates.io/crates/depup-cli) as `depup-cli` and installs the `depup` binary.

## Precompiled Binaries

Download the latest release from [GitHub Releases](https://github.com/hpehl/depup/releases) for your platform:

| Platform | Target |
|----------|--------|
| macOS (Apple Silicon) | `aarch64-apple-darwin` |
| macOS (Intel) | `x86_64-apple-darwin` |
| Linux (x64) | `x86_64-unknown-linux-gnu` |
| Windows (x64) | `x86_64-pc-windows-msvc` |

Extract the archive and place the `depup` binary somewhere in your `$PATH`.

## Build from Source

1. [Install Rust and Cargo](https://www.rust-lang.org/tools/install)
2. Clone the repository:
   ```bash
   git clone git@github.com:hpehl/depup.git
   cd depup
   ```
3. Build and install:
   ```bash
   cargo build --release && cargo install --path .
   ```

This installs the `depup` binary to `~/.cargo/bin/` which should be in your `$PATH`.

## Requirements

- Rust 1.85+ (edition 2024) — only needed when building from source
- Network access to Maven Central (`repo1.maven.org`) and any custom repositories defined in the project's POMs
- For npm ecosystem checks: the respective package manager (npm, pnpm, yarn, or bun) must be installed and on PATH
