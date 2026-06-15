mod app;
mod args;
mod command;
mod constants;
mod error;
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
        Some(("check", m)) => command::check::check(m).await,
        Some(("update", m)) => command::update::update(m),
        Some(("audit", m)) => command::audit::audit(m),
        Some(("completions", m)) => command::completions::completions(m),
        _ => command::check::check(&matches).await,
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
