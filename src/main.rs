mod app;
mod args;
mod command;
mod constants;
mod error;
mod json;
mod maven;
mod output;
mod pnpm;
mod progress;
mod registry;
mod version;

use anyhow::Result;

use crate::error::{JsonErrorEnvelope, DepupError};

#[tokio::main]
async fn main() {
    clap_complete::CompleteEnv::with_factory(app::build_app).complete();

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

async fn run() -> Result<()> {
    let matches = app::build_app()
        .try_get_matches()
        .map_err(classify_clap_error)?;

    match matches.subcommand() {
        Some(("maven", m)) => match m.subcommand() {
            Some(("check", m)) => command::check::maven_check(m).await,
            _ => command::check::maven_check(m).await,
        },
        Some(("pnpm", m)) => match m.subcommand() {
            Some(("check", m)) => command::check::pnpm_check(m).await,
            _ => command::check::pnpm_check(m).await,
        },
        Some(("completions", m)) => command::completions::completions(m),
        Some(("check", m)) => command::check::auto_check(m).await,
        _ => command::check::auto_check(&matches).await,
    }
}

#[allow(clippy::needless_pass_by_value)]
fn classify_clap_error(err: clap::Error) -> anyhow::Error {
    match err.kind() {
        clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
            err.exit();
        }
        _ => DepupError::clap_parse_error(err.to_string().trim()).into(),
    }
}
