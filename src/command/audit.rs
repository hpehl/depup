use anyhow::Result;
use clap::ArgMatches;
use console::style;

use crate::args;

#[allow(clippy::unnecessary_wraps)]
pub fn audit(matches: &ArgMatches) -> Result<()> {
    let json = args::is_json(matches);
    if json {
        println!(
            "{}",
            serde_json::json!({
                "error": {
                    "code": "NOT_IMPLEMENTED",
                    "message": "The audit command is not yet implemented"
                }
            })
        );
    } else {
        println!(
            "{} The {} command is not yet implemented.",
            style("!").yellow().bold(),
            style("audit").bold()
        );
    }
    Ok(())
}
