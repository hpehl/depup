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
        .after_long_help(
            "Exit codes:\n  \
             0  All dependencies are up to date / no vulnerabilities\n  \
             1  Outdated dependencies found (check) or update errors (update)\n  \
             2  Vulnerabilities found (audit)\n  \
             3  Critical or high severity vulnerabilities found (audit)",
        )
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
            .short('s')
            .long("stable")
            .visible_alias("releases-only")
            .action(ArgAction::SetTrue)
            .help("Exclude pre-release versions (alpha, beta, CR, RC, milestone)"),
    )
    .arg(
        Arg::new("managed")
            .short('M')
            .long("managed")
            .action(ArgAction::SetTrue)
            .conflicts_with("unmanaged")
            .help("Only show dependencies with a version property (Maven only)"),
    )
    .arg(
        Arg::new("unmanaged")
            .short('U')
            .long("unmanaged")
            .action(ArgAction::SetTrue)
            .help("Only show dependencies without a version property (Maven only)"),
    )
    .arg(
        Arg::new("maven")
            .short('m')
            .long("maven")
            .action(ArgAction::SetTrue)
            .conflicts_with("npm")
            .help("Only show Maven ecosystem results"),
    )
    .arg(
        Arg::new("npm")
            .short('n')
            .long("npm")
            .action(ArgAction::SetTrue)
            .help("Only show npm ecosystem results"),
    )
}

fn kind_args(cmd: Command, include_tools: bool) -> Command {
    let mut cmd = cmd
        .arg(
            Arg::new("dependencies")
                .short('d')
                .long("dependencies")
                .action(ArgAction::SetTrue)
                .help("Only show dependencies (combinable with other kind flags)"),
        )
        .arg(
            Arg::new("plugins")
                .short('p')
                .long("plugins")
                .action(ArgAction::SetTrue)
                .help("Only show plugins (combinable with other kind flags)"),
        )
        .arg(
            Arg::new("dev-dependencies")
                .short('D')
                .long("dev-dependencies")
                .alias("dev-deps")
                .action(ArgAction::SetTrue)
                .help("Only show dev dependencies (combinable with other kind flags)"),
        );

    if include_tools {
        cmd = cmd.arg(
            Arg::new("tools")
                .short('t')
                .long("tools")
                .visible_alias("other")
                .action(ArgAction::SetTrue)
                .help("Only show tool version checks (combinable with other kind flags)"),
        );
    }
    cmd
}

fn check_args(cmd: Command) -> Command {
    kind_args(common_filter_args(cmd), true).arg(
        Arg::new("outdated")
            .short('o')
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
                .short('v')
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
    fn kind_filters_combinable() {
        assert!(parse(&["depup", "check", "--dependencies", "--plugins"]).is_ok());
        assert!(parse(&["depup", "check", "--dependencies", "--tools"]).is_ok());
        assert!(parse(&["depup", "check", "--plugins", "--dev-dependencies"]).is_ok());
        assert!(parse(&["depup", "check", "--dependencies", "--plugins", "--tools"]).is_ok());
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
