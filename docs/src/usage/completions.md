# Completions

The `completions` subcommand generates and installs shell completions for tab-completion of subcommands and flags.

```bash
depup completions [OPTIONS] [SHELL]
```

## Generate Completions

```bash
# Auto-detect shell and print completions to stdout
depup completions

# Generate completions for a specific shell
depup completions fish
depup completions bash
depup completions zsh
```

## Install Completions

```bash
# Auto-detect shell, install to standard path
depup completions --install

# Install completions for a specific shell
depup completions --install zsh
depup completions --install fish
```

## Supported Shells

| Shell | Description |
|-------|-------------|
| bash | Bourne Again Shell |
| zsh | Z Shell |
| fish | Friendly Interactive Shell |
| elvish | Elvish Shell |
| powershell | PowerShell |

If no shell is specified, `depup` auto-detects your current shell from the environment.
