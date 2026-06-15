//! Orchestrates check pipelines across all ecosystems.
//!
//! Auto-detects Maven (via `pom.xml`) and npm (via lockfiles or `packageManager` field),
//! runs all discovered checks concurrently via the shared pipeline, applies CLI filters,
//! and outputs results as a table or JSON. Exits with code 1 if any outdated
//! dependencies are found.

use anyhow::Result;
use clap::ArgMatches;
use tokio::time::Instant;

use crate::app;
use crate::filter::Filter;
use crate::output;
use crate::progress;
use crate::dependency::{VersionResult, Ecosystem};

/// Main entry point for the `check` subcommand.
pub async fn check(matches: &ArgMatches) -> Result<()> {
    let path = app::path_argument(matches);
    let json = app::is_json(matches);
    let filter = Filter::from_matches(matches);

    let instant = Instant::now();
    let root = path.canonicalize().unwrap_or_else(|_| path.clone());

    let do_maven =
        filter.ecosystem.is_none_or(|e| e != Ecosystem::Npm) && root.join("pom.xml").exists();
    let do_npm = filter.ecosystem.is_none_or(|e| e != Ecosystem::Maven);

    let (all_results, _npm_projects) =
        super::pipeline::run_checks(&root, do_maven, do_npm, filter.stable, json).await?;

    if all_results.is_empty() {
        if json {
            println!("[]");
        } else {
            println!("No supported project found.");
        }
        return Ok(());
    }

    let filtered: Vec<VersionResult> = all_results
        .into_iter()
        .filter(|r| filter.matches(r))
        .collect();

    if json {
        output::print_json(&filtered);
    } else {
        println!();
        output::print_results(&filtered);
        progress::done(instant);
    }

    if filtered.iter().any(|r| r.is_outdated()) {
        std::process::exit(1);
    }

    Ok(())
}
