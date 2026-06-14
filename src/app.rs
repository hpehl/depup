use clap::builder::Styles;
use clap::builder::styling::{AnsiColor, Effects};
use clap::{Arg, ArgAction, Command, crate_name, crate_version};

pub fn build_app() -> Command {
    let styles = Styles::styled()
        .header(AnsiColor::Green.on_default() | Effects::BOLD)
        .usage(AnsiColor::Green.on_default() | Effects::BOLD)
        .literal(AnsiColor::Blue.on_default() | Effects::BOLD)
        .placeholder(AnsiColor::Cyan.on_default());

    let app = Command::new(crate_name!())
        .version(crate_version!())
        .about("Check dependency versions across multiple ecosystems")
        .styles(styles)
        .propagate_version(true)
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
        );
    check_args(app)
}

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
        Arg::new("include-pre-releases")
            .long("include-pre-releases")
            .action(ArgAction::SetTrue)
            .help("Include pre-release versions (alpha, beta, CR, RC, milestone)"),
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
