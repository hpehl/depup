//! The `audit` subcommand: checks dependencies for known vulnerabilities via OSV.dev.
//!
//! Reuses the check pipeline to discover dependencies and their versions,
//! then queries OSV.dev for known vulnerabilities. Supports the same
//! ecosystem/kind/include/exclude filters as check, plus severity filtering.

mod osv;

use anyhow::Result;
use clap::ArgMatches;
use indicatif::ProgressBar;
use tokio::time::Instant;

use crate::app;
use crate::dependency::{AuditResult, DependencyInfo, DependencyKind, VersionResult};
use crate::filter::Filter;
use crate::json::AuditJsonResult;
use crate::output;
use crate::progress;

/// Returns `true` if the process should exit with code 1 (vulnerabilities found).
pub async fn audit(matches: &ArgMatches) -> Result<bool> {
    let path = app::path_argument(matches);
    let json = app::is_json(matches);
    let filter = Filter::from_matches(matches);

    let instant = Instant::now();
    let root = path.canonicalize().unwrap_or_else(|_| path.clone());

    let (do_maven, do_npm) = super::pipeline::detect_ecosystems(&filter, &root);

    // Phase 1: Discover and check dependencies (reuse check pipeline for version info)
    let (check_results, _npm_projects) =
        crate::command::pipeline::resolve_versions(&root, do_maven, do_npm, filter.stable, json)
            .await?;

    // Filter to deps matching the user's filters (excluding tool versions)
    let auditable: Vec<VersionResult> = check_results
        .into_iter()
        .filter(|r| r.kind() != DependencyKind::ToolVersion && filter.matches(r))
        .collect();

    if auditable.is_empty() {
        if json {
            println!("[]");
        } else {
            println!("No dependencies to audit.");
        }
        return Ok(false);
    }

    // Phase 2: Query OSV.dev for vulnerabilities
    let bar = if json {
        ProgressBar::hidden()
    } else {
        progress::bar(2)
    };

    let audit_results = osv::audit(&auditable, &bar).await?;
    bar.finish_and_clear();

    // Phase 3: Apply severity filter
    let filtered: Vec<AuditResult> = apply_severity_filter(audit_results, &filter);

    // Phase 4: Output
    if json {
        let json_results: Vec<AuditJsonResult> =
            filtered.iter().map(AuditJsonResult::from).collect();
        output::print_json(&json_results);
    } else {
        println!();
        output::print_table(&filtered, "No dependencies to show.", output::audit_summary);
        progress::done(instant);
    }

    Ok(filtered.iter().any(|r| r.is_vulnerable()))
}

fn apply_severity_filter(results: Vec<AuditResult>, filter: &Filter) -> Vec<AuditResult> {
    if filter.severity.is_none() {
        return results;
    }

    results
        .into_iter()
        .map(|r| AuditResult {
            vulnerabilities: r
                .vulnerabilities
                .into_iter()
                .filter(|v| filter.matches_severity(v.severity))
                .collect(),
            ..r
        })
        .collect()
}
