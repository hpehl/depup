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
        .about("Check dependency versions across Maven and pnpm ecosystems")
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
            Command::new("maven")
                .about("Maven ecosystem commands")
                .subcommand(
                    maven_check_args(Command::new("check"))
                        .about("Check Maven version properties against upstream registries"),
                ),
        )
        .subcommand(
            Command::new("pnpm")
                .about("pnpm ecosystem commands")
                .subcommand(
                    pnpm_check_args(Command::new("check"))
                        .about("Check pnpm projects for outdated packages"),
                ),
        )
        .subcommand(
            auto_check_args(Command::new("check"))
                .about("Auto-detect ecosystem and check for outdated dependencies"),
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
    auto_check_args(app)
}

fn maven_check_args(cmd: Command) -> Command {
    cmd.arg(
        Arg::new("path")
            .default_value(".")
            .help("Path to the Maven project root"),
    )
    .arg(
        Arg::new("outdated")
            .long("outdated")
            .action(ArgAction::SetTrue)
            .help("Only show outdated properties"),
    )
    .arg(
        Arg::new("include-pre-releases")
            .long("include-pre-releases")
            .action(ArgAction::SetTrue)
            .help("Include pre-release versions (alpha, beta, CR, RC, milestone)"),
    )
}

fn pnpm_check_args(cmd: Command) -> Command {
    cmd.arg(
        Arg::new("path")
            .default_value(".")
            .help("Root directory to search for pnpm projects"),
    )
}

fn auto_check_args(cmd: Command) -> Command {
    cmd.arg(
        Arg::new("path")
            .default_value(".")
            .help("Path to the project root (auto-detects ecosystem)"),
    )
    .arg(
        Arg::new("outdated")
            .long("outdated")
            .action(ArgAction::SetTrue)
            .help("Only show outdated properties (Maven only)"),
    )
    .arg(
        Arg::new("include-pre-releases")
            .long("include-pre-releases")
            .action(ArgAction::SetTrue)
            .help("Include pre-release versions (Maven only)"),
    )
}
