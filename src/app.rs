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

/// Adds arguments shared by check, update, and audit: path, include/exclude,
/// stable, managed/unmanaged, and ecosystem filters.
fn common_filter_args(cmd: Command) -> Command {
    cmd.arg(
        Arg::new("path")
            .default_value(".")
            .help("Path to the project root (auto-detects ecosystems)"),
    )
    .arg(
        Arg::new("include")
            .long("include")
            .action(ArgAction::Append)
            .help("Only include artifacts matching a glob pattern (e.g., 'org.junit:*', 'react*')"),
    )
    .arg(
        Arg::new("exclude")
            .long("exclude")
            .action(ArgAction::Append)
            .help("Exclude artifacts matching a glob pattern (e.g., '*:guava', '@scope/*')"),
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
}

fn kind_args(cmd: Command, include_tools: bool) -> Command {
    let mut deps_conflicts = vec!["plugins", "dev-deps"];
    let mut plugins_conflicts: Vec<&str> = vec!["dev-deps"];
    if include_tools {
        deps_conflicts.push("tools");
        plugins_conflicts.push("tools");
    }

    let mut cmd = cmd
        .arg(
            Arg::new("dependencies")
                .long("dependencies")
                .action(ArgAction::SetTrue)
                .conflicts_with_all(deps_conflicts)
                .help("Only show dependencies"),
        )
        .arg(
            Arg::new("plugins")
                .long("plugins")
                .action(ArgAction::SetTrue)
                .conflicts_with_all(plugins_conflicts)
                .help("Only show plugins"),
        )
        .arg({
            let mut arg = Arg::new("dev-deps")
                .long("dev-deps")
                .action(ArgAction::SetTrue)
                .help("Only show dev dependencies");
            if include_tools {
                arg = arg.conflicts_with("tools");
            }
            arg
        });

    if include_tools {
        cmd = cmd.arg(
            Arg::new("tools")
                .long("tools")
                .visible_alias("other")
                .action(ArgAction::SetTrue)
                .help("Only show tool version checks (Node.js, package manager versions)"),
        );
    }
    cmd
}

fn check_args(cmd: Command) -> Command {
    kind_args(common_filter_args(cmd), true).arg(
        Arg::new("outdated")
            .long("outdated")
            .action(ArgAction::SetTrue)
            .help("Only show outdated dependencies"),
    )
}

fn update_args(cmd: Command) -> Command {
    kind_args(common_filter_args(cmd), true).arg(
        Arg::new("dry-run")
            .long("dry-run")
            .action(ArgAction::SetTrue)
            .help("Show what would be updated without making changes"),
    )
}

fn audit_args(cmd: Command) -> Command {
    kind_args(common_filter_args(cmd), false)
        .arg(
            Arg::new("vulnerable")
                .long("vulnerable")
                .action(ArgAction::SetTrue)
                .help("Only show dependencies with known vulnerabilities"),
        )
        .arg(
            Arg::new("severity")
                .long("severity")
                .value_parser(["critical", "high", "medium", "low"])
                .help("Only show vulnerabilities at or above this severity level"),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<ArgMatches, clap::Error> {
        build_app().try_get_matches_from(args)
    }

    #[test]
    fn check_subcommand_parses() {
        let m = parse(&["depup", "check", "/some/path"]).unwrap();
        let sub = m.subcommand_matches("check").unwrap();
        assert_eq!(path_argument(sub), PathBuf::from("/some/path"));
    }

    #[test]
    fn path_defaults_to_current_dir() {
        let m = parse(&["depup", "check"]).unwrap();
        let sub = m.subcommand_matches("check").unwrap();
        assert_eq!(path_argument(sub), PathBuf::from("."));
    }

    #[test]
    fn json_flag_global() {
        let m = parse(&["depup", "--json", "check"]).unwrap();
        assert!(is_json(&m));
    }

    #[test]
    fn maven_npm_conflict_rejected() {
        let result = parse(&["depup", "check", "--maven", "--npm"]);
        assert!(result.is_err());
    }

    #[test]
    fn managed_unmanaged_conflict_rejected() {
        let result = parse(&["depup", "check", "--managed", "--unmanaged"]);
        assert!(result.is_err());
    }

    #[test]
    fn kind_filter_conflicts_rejected() {
        assert!(parse(&["depup", "check", "--dependencies", "--plugins"]).is_err());
        assert!(parse(&["depup", "check", "--dependencies", "--tools"]).is_err());
        assert!(parse(&["depup", "check", "--plugins", "--dev-deps"]).is_err());
    }

    #[test]
    fn audit_has_no_tools_flag() {
        let result = parse(&["depup", "audit", "--tools"]);
        assert!(result.is_err());
    }

    #[test]
    fn audit_severity_valid_values() {
        assert!(parse(&["depup", "audit", "--severity", "critical"]).is_ok());
        assert!(parse(&["depup", "audit", "--severity", "high"]).is_ok());
        assert!(parse(&["depup", "audit", "--severity", "medium"]).is_ok());
        assert!(parse(&["depup", "audit", "--severity", "low"]).is_ok());
        assert!(parse(&["depup", "audit", "--severity", "invalid"]).is_err());
    }

    #[test]
    fn update_dry_run_flag() {
        let m = parse(&["depup", "update", "--dry-run"]).unwrap();
        let sub = m.subcommand_matches("update").unwrap();
        assert!(sub.get_flag("dry-run"));
    }

    #[test]
    fn check_outdated_flag() {
        let m = parse(&["depup", "check", "--outdated"]).unwrap();
        let sub = m.subcommand_matches("check").unwrap();
        assert!(sub.get_flag("outdated"));
    }

    #[test]
    fn subcommand_required() {
        let result = parse(&["depup"]);
        assert!(result.is_err());
    }
}
