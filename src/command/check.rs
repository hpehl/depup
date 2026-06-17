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
use crate::json::JsonResult;
use crate::model::CheckResult;
use crate::output;
use crate::progress;

/// Main entry point for the `check` subcommand.
/// Returns `true` if the process should exit with code 1 (outdated deps found).
pub async fn check(matches: &ArgMatches) -> Result<bool> {
    let path = app::path_argument(matches);
    let json = app::is_json(matches);
    let filter = Filter::from_matches(matches);

    let instant = Instant::now();
    let root = path.canonicalize().unwrap_or_else(|_| path.clone());

    let (do_maven, do_npm) = super::pipeline::detect_ecosystems(&filter, &root);

    let (all_results, _npm_projects) =
        super::pipeline::resolve_versions(&root, do_maven, do_npm, filter.stable, json).await?;

    if all_results.is_empty() {
        if json {
            println!("[]");
        } else {
            println!("No supported project found.");
        }
        return Ok(false);
    }

    let filtered: Vec<CheckResult> = all_results
        .into_iter()
        .filter(|r| filter.matches(r))
        .collect();

    if json {
        let json_results: Vec<JsonResult> = filtered.iter().map(JsonResult::from).collect();
        output::print_json(&json_results);
    } else {
        println!();
        println!();
        output::print_table(&filtered, "No dependencies to show.", output::check_summary);
        progress::done(instant);
    }

    Ok(filtered.iter().any(|r| r.is_outdated()))
}
