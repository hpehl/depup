use clap::builder::Styles;
use clap::builder::styling::{AnsiColor, Effects};
use clap::{Arg, ArgAction, Command, crate_name, crate_version};

pub fn build_app() -> Command {
    let app = Command::new(crate_name!())
        .version(crate_version!())
        .about("Check Maven version properties against upstream registries")
        .styles(
            Styles::styled()
                .header(AnsiColor::Green.on_default() | Effects::BOLD)
                .usage(AnsiColor::Green.on_default() | Effects::BOLD)
                .literal(AnsiColor::Blue.on_default() | Effects::BOLD)
                .placeholder(AnsiColor::Cyan.on_default()),
        )
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
                .about("Check version properties against upstream registries (default command)"),
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
