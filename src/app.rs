//! CLI definition using the clap builder API.
//!
//! Separated from `main.rs` so the shell completion system can build the command
//! tree independently without pulling in runtime dependencies.

use std::path::PathBuf;

use clap::builder::Styles;
use clap::builder::styling::{AnsiColor, Effects};
use clap::{Arg, ArgAction, ArgMatches, Command, crate_version};

/// Extracts the project root path from CLI arguments, defaulting to the current directory.
pub fn path_argument(matches: &ArgMatches) -> PathBuf {
    matches
        .get_one::<String>("path")
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
}

/// Returns whether the global `--json` flag is set.
pub fn is_json(matches: &ArgMatches) -> bool {
    matches.get_flag("json")
}

/// Builds the complete clap `Command` tree with styled help text.
pub fn build_app() -> Command {
    let styles = Styles::styled()
        .header(AnsiColor::Green.on_default() | Effects::BOLD)
        .usage(AnsiColor::Green.on_default() | Effects::BOLD)
        .literal(AnsiColor::Blue.on_default() | Effects::BOLD)
        .placeholder(AnsiColor::Cyan.on_default());

    Command::new("depup")
        .version(crate_version!())
        .about("Check dependency versions across multiple ecosystems")
        .styles(styles)
        .propagate_version(true)
        .subcommand_required(true)
        .arg(
            Arg::new("json")
                .long("json")
                .global(true)
                .action(ArgAction::SetTrue)
                .help("Output results as JSON (for machine consumption)"),
        )
        .subcommand(
            check_args(Command::new("check"))
                .about("Check for outdated dependencies (auto-detects ecosystems)"),
        )
        .subcommand(
            update_args(Command::new("update"))
                .about("Update dependencies to their latest versions"),
        )
        .subcommand(
            audit_args(Command::new("audit")).about("Audit dependencies for known vulnerabilities"),
        )
        .subcommand(
            Command::new("completions")
                .about("Generate and install shell completions")
                .arg(
                    Arg::new("shell")
                        .help("The shell to generate completions for [default: auto-detected]"),
                )
                .arg(
                    Arg::new("install")
                        .short('i')
                        .long("install")
                        .action(ArgAction::SetTrue)
                        .help("Install completions to the standard location for the shell"),
                ),
        )
}

/// Adds check-specific arguments: path, filtering, and ecosystem selection flags.
fn check_args(cmd: Command) -> Command {
    cmd.arg(
        Arg::new("path")
            .default_value(".")
            .help("Path to the project root (auto-detects ecosystems)"),
    )
    .arg(
        Arg::new("outdated")
            .long("outdated")
            .action(ArgAction::SetTrue)
            .help("Only show outdated dependencies"),
    )
    .arg(
        Arg::new("stable")
            .long("stable")
            .visible_alias("releases-only")
            .action(ArgAction::SetTrue)
            .help("Exclude pre-release versions (alpha, beta, CR, RC, milestone)"),
    )
    .arg(
        Arg::new("managed")
            .long("managed")
            .action(ArgAction::SetTrue)
            .conflicts_with("unmanaged")
            .help("Only show dependencies with a version property (Maven only)"),
    )
    .arg(
        Arg::new("unmanaged")
            .long("unmanaged")
            .action(ArgAction::SetTrue)
            .help("Only show dependencies without a version property (Maven only)"),
    )
    .arg(
        Arg::new("maven")
            .long("maven")
            .action(ArgAction::SetTrue)
            .conflicts_with("npm")
            .help("Only show Maven ecosystem results"),
    )
    .arg(
        Arg::new("npm")
            .long("npm")
            .action(ArgAction::SetTrue)
            .help("Only show npm ecosystem results"),
    )
    .arg(
        Arg::new("dependencies")
            .long("dependencies")
            .action(ArgAction::SetTrue)
            .conflicts_with_all(["plugins", "dev-deps", "tools"])
            .help("Only show dependencies"),
    )
    .arg(
        Arg::new("plugins")
            .long("plugins")
            .action(ArgAction::SetTrue)
            .conflicts_with_all(["dev-deps", "tools"])
            .help("Only show plugins"),
    )
    .arg(
        Arg::new("dev-deps")
            .long("dev-deps")
            .action(ArgAction::SetTrue)
            .conflicts_with("tools")
            .help("Only show dev dependencies"),
    )
    .arg(
        Arg::new("tools")
            .long("tools")
            .visible_alias("other")
            .action(ArgAction::SetTrue)
            .help("Only show tool version checks (Node.js, package manager versions)"),
    )
}

fn update_args(cmd: Command) -> Command {
    cmd.arg(
        Arg::new("path")
            .default_value(".")
            .help("Path to the project root"),
    )
}

fn audit_args(cmd: Command) -> Command {
    cmd.arg(
        Arg::new("path")
            .default_value(".")
            .help("Path to the project root"),
    )
}
