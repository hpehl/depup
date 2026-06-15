pub mod audit;
pub mod check;
pub mod completions;
pub mod update;

use anyhow::Result;
use clap::ArgMatches;
use console::style;

use crate::args;

#[allow(clippy::unnecessary_wraps)]
fn not_implemented(command: &str, matches: &ArgMatches) -> Result<()> {
    if args::is_json(matches) {
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
