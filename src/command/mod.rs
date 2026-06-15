//! Subcommand handlers for the CLI.
//!
//! Each subcommand lives in its own module. The `not_implemented` helper
//! provides a consistent "not yet implemented" message for stub subcommands.

pub mod audit;
pub mod check;
pub mod completions;
pub mod pipeline;
pub mod update;

use anyhow::Result;
use clap::ArgMatches;
use console::style;

use crate::app;

/// Prints a "not yet implemented" message for stub subcommands.
/// Respects `--json` mode for machine-consumable output.
#[allow(clippy::unnecessary_wraps)]
fn not_implemented(command: &str, matches: &ArgMatches) -> Result<()> {
    if app::is_json(matches) {
        println!(
            "{}",
            serde_json::json!({
                "error": {
                    "code": "NOT_IMPLEMENTED",
                    "message": format!("The {command} command is not yet implemented")
                }
            })
        );
    } else {
        println!(
            "{} The {} command is not yet implemented.",
            style("!").yellow().bold(),
            style(command).bold()
        );
    }
    Ok(())
}
