//! depup — CLI tool for checking dependency versions across multiple ecosystems.
//!
//! Supports Maven and npm ecosystems with auto-detection based on project files.
//! The pipeline flows: Discovery → Check → Comparison → Output.

mod app;
mod command;
mod constants;
mod error;
mod filter;
mod json;
mod maven;
mod npm;
mod output;
mod progress;
mod registry;
mod version;

use anyhow::Result;

use crate::error::{DepupError, JsonErrorEnvelope};

#[tokio::main]
async fn main() {
    // Enable dynamic shell completions via clap_complete's `CompleteEnv` protocol.
    clap_complete::CompleteEnv::with_factory(app::build_app).complete();

    // Pre-check for --json flag before parsing to format top-level errors correctly.
    let json = std::env::args().any(|a| a == "--json");
    if let Err(e) = run().await {
        if json {
            let envelope = JsonErrorEnvelope::from_anyhow(&e);
            match serde_json::to_string(&envelope) {
                Ok(json) => println!("{json}"),
                Err(ser) => eprintln!("Error: {e:#}\n(JSON serialization also failed: {ser})"),
            }
        } else {
            eprintln!("Error: {e:#}");
        }
        std::process::exit(1);
    }
}

/// Parses CLI arguments and dispatches to the appropriate subcommand handler.
async fn run() -> Result<()> {
    let matches = app::build_app()
        .try_get_matches()
        .map_err(classify_clap_error)?;

    match matches.subcommand() {
        Some(("check", m)) => command::check::check(m).await,
        Some(("update", m)) => command::update::update(m),
        Some(("audit", m)) => command::audit::audit(m),
        Some(("completions", m)) => command::completions::completions(m),
        _ => unreachable!("subcommand_required is set"),
    }
}

/// Converts clap errors into `DepupError` for structured output.
/// Help and version display errors are handled by clap directly (exit 0).
#[allow(clippy::needless_pass_by_value)]
fn classify_clap_error(err: clap::Error) -> anyhow::Error {
    match err.kind() {
        clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
            err.exit();
        }
        _ => DepupError::clap_parse_error(err.to_string().trim()).into(),
    }
}
