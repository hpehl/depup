mod app;
mod args;
mod command;
mod constants;
mod discovery;
mod error;
mod json;
mod output;
mod pom;
mod progress;
mod registry;
mod version;

use anyhow::Result;

use crate::error::{JsonErrorEnvelope, MvnupError};

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
        Some(("completions", m)) => command::completions::completions(m),
        Some(("check", m)) => command::check::check(m).await,
        _ => command::check::check(&matches).await,
    }
}

fn classify_clap_error(err: clap::Error) -> anyhow::Error {
    match err.kind() {
        clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
            err.exit();
        }
        _ => MvnupError::clap_parse_error(err.to_string().trim()).into(),
    }
}
