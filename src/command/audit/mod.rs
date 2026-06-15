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
use crate::dependency::{AuditResult, DependencyKind, VersionResult};
use crate::filter::Filter;
use crate::output;
use crate::progress;

pub async fn audit(matches: &ArgMatches) -> Result<()> {
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
        return Ok(());
    }

    // Phase 2: Query OSV.dev for vulnerabilities
    // Progress bar: 2 steps (batch query + detail fetch)
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
        output::print_audit_json(&filtered);
    } else {
        println!();
        output::print_audit_results(&filtered);
        progress::done(instant);
    }

    if filtered.iter().any(|r| r.is_vulnerable()) {
        std::process::exit(1);
    }

    Ok(())
}

fn apply_severity_filter(results: Vec<AuditResult>, filter: &Filter) -> Vec<AuditResult> {
    if filter.severity.is_none() {
        return results;
    }

    results
        .into_iter()
        .map(|mut r| {
            r.vulnerabilities
                .retain(|v| filter.matches_severity(v.severity));
            r
        })
        .collect()
}
