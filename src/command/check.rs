//! Orchestrates check pipelines across all ecosystems.
//!
//! Auto-detects Maven (via `pom.xml`) and npm (via lockfiles or `packageManager` field),
//! runs all discovered checks concurrently via the shared pipeline, applies CLI filters,
//! and outputs results as a table or JSON. Exits with code 1 if any outdated
//! dependencies are found.

use anyhow::Result;
use clap::ArgMatches;
use tokio::time::Instant;

use crate::model::CheckResult;
use crate::output;
use crate::output::json::JsonResult;
use crate::progress;

/// Main entry point for the `check` subcommand.
/// Returns exit code 1 if outdated dependencies are found, 0 otherwise.
pub async fn check(matches: &ArgMatches) -> Result<u8> {
    let setup = super::pipeline::CommandSetup::from_matches(matches);
    let instant = Instant::now();

    let pipeline = super::pipeline::resolve_versions(&setup.resolve_config()).await?;
    let all_results = pipeline.results;

    if all_results.is_empty() {
        super::pipeline::print_empty(setup.json, "No supported project found.");
        return Ok(0);
    }

    let filtered: Vec<CheckResult> = all_results
        .into_iter()
        .filter(|r| setup.filter.matches(r))
        .collect();

    if setup.json {
        let json_results: Vec<JsonResult> = filtered.iter().map(JsonResult::from).collect();
        output::print_json(&json_results);
    } else {
        println!();
        println!();
        output::print_table(&filtered, "No dependencies to show.", output::check_summary);
        progress::done(instant);
    }

    if filtered.iter().any(|r| r.is_outdated()) {
        Ok(1)
    } else {
        Ok(0)
    }
}
