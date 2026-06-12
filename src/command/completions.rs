use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use clap::ArgMatches;

const SUPPORTED_SHELLS: &[&str] = &["bash", "zsh", "fish", "elvish", "powershell"];

pub fn completions(matches: &ArgMatches) -> Result<()> {
    let shell = matches
        .get_one::<String>("shell")
        .map_or_else(|| detect_shell(), String::as_str);

    if !SUPPORTED_SHELLS.contains(&shell) {
        bail!(
            "Unsupported shell: '{}'. Supported shells: {}",
            shell,
            SUPPORTED_SHELLS.join(", ")
        );
    }

    if matches.get_flag("install") {
        install_completions(shell)
    } else {
        print_completions(shell)
    }
}

fn detect_shell() -> &'static str {
    if let Ok(shell) = env::var("SHELL") {
        if shell.contains("fish") {
            return "fish";
        } else if shell.contains("zsh") {
            return "zsh";
        } else if shell.contains("bash") {
            return "bash";
        } else if shell.contains("elvish") {
            return "elvish";
        }
    }
    if env::var("PSModulePath").is_ok() {
        return "powershell";
    }
    "bash"
}

fn generate_script(shell: &str) -> Result<Vec<u8>> {
    let exe = env::current_exe().with_context(|| "Could not determine executable path")?;
    let output = Command::new(&exe)
        .env("COMPLETE", shell)
        .output()
        .with_context(|| format!("Failed to run '{}' with COMPLETE={}", exe.display(), shell))?;
    if !output.status.success() {
        bail!(
            "Failed to generate completions for {}: {}",
            shell,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(output.stdout)
}

fn print_completions(shell: &str) -> Result<()> {
    let script = generate_script(shell)?;
    io::stdout()
        .write_all(&script)
        .with_context(|| "Failed to write to stdout")?;
    Ok(())
}

fn install_completions(shell: &str) -> Result<()> {
    let script = generate_script(shell)?;
    let path = completion_path(shell)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory '{}'", parent.display()))?;
    }
    fs::write(&path, &script)
        .with_context(|| format!("Failed to write completions to '{}'", path.display()))?;

    println!("Completions installed to {}", path.display());
    print_post_install_instructions(shell);
    Ok(())
}

fn completion_path(shell: &str) -> Result<PathBuf> {
    let home = home_dir().with_context(|| "Could not determine home directory")?;
    completion_path_for_home(&home, shell)
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn completion_path_for_home(home: &Path, shell: &str) -> Result<PathBuf> {
    match shell {
        "fish" => Ok(home.join(".config/fish/completions/mvnup.fish")),
        "zsh" => Ok(home.join(".zsh/completions/_mvnup")),
        "bash" => Ok(home.join(".local/share/bash-completion/completions/mvnup")),
        "elvish" => Ok(home.join(".config/elvish/lib/mvnup.elv")),
        "powershell" => Ok(home.join(".config/powershell/mvnup.ps1")),
        _ => bail!("Unsupported shell: {shell}"),
    }
}

fn print_post_install_instructions(shell: &str) {
    match shell {
        "fish" => {
            println!("Fish completions are loaded automatically from this location.");
        }
        "bash" => {
            println!(
                "\nIf completions are not loaded automatically, add this to your ~/.bashrc:\n  \
                 source ~/.local/share/bash-completion/completions/mvnup"
            );
        }
        "zsh" => {
            println!(
                "\nMake sure ~/.zsh/completions is in your fpath. Add this to your ~/.zshrc \
                 (before compinit):\n  fpath=(~/.zsh/completions $fpath)\n  autoload -U compinit \
                 && compinit"
            );
        }
        "elvish" => {
            println!("\nAdd this to your ~/.config/elvish/rc.elv:\n  use mvnup");
        }
        "powershell" => {
            println!("\nAdd this to your PowerShell profile:\n  . ~/.config/powershell/mvnup.ps1");
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_path_fish() {
        let home = PathBuf::from("/home/user");
        let path = completion_path_for_home(&home, "fish").unwrap();
        assert_eq!(
            path,
            PathBuf::from("/home/user/.config/fish/completions/mvnup.fish")
        );
    }

    #[test]
    fn completion_path_zsh() {
        let home = PathBuf::from("/home/user");
        let path = completion_path_for_home(&home, "zsh").unwrap();
        assert_eq!(path, PathBuf::from("/home/user/.zsh/completions/_mvnup"));
    }

    #[test]
    fn completion_path_bash() {
        let home = PathBuf::from("/home/user");
        let path = completion_path_for_home(&home, "bash").unwrap();
        assert_eq!(
            path,
            PathBuf::from("/home/user/.local/share/bash-completion/completions/mvnup")
        );
    }

    #[test]
    fn completion_path_elvish() {
        let home = PathBuf::from("/home/user");
        let path = completion_path_for_home(&home, "elvish").unwrap();
        assert_eq!(
            path,
            PathBuf::from("/home/user/.config/elvish/lib/mvnup.elv")
        );
    }

    #[test]
    fn completion_path_powershell() {
        let home = PathBuf::from("/home/user");
        let path = completion_path_for_home(&home, "powershell").unwrap();
        assert_eq!(
            path,
            PathBuf::from("/home/user/.config/powershell/mvnup.ps1")
        );
    }

    #[test]
    fn completion_path_unsupported_shell_fails() {
        let home = PathBuf::from("/home/user");
        assert!(completion_path_for_home(&home, "nushell").is_err());
    }
}
