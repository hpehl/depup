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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dependency::{Dependency, Ecosystem, Severity, Vulnerability};

    fn no_filter() -> Filter {
        Filter {
            outdated: false,
            stable: false,
            managed: None,
            ecosystem: None,
            kind: None,
            include: Vec::new(),
            exclude: Vec::new(),
            severity: None,
        }
    }

    fn make_audit_result(vulns: Vec<Vulnerability>) -> AuditResult {
        AuditResult {
            id: Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                "org.example:lib".into(),
                None,
                String::new(),
            ),
            version: "1.0.0".into(),
            vulnerabilities: vulns,
        }
    }

    fn make_vuln(id: &str, severity: Severity) -> Vulnerability {
        Vulnerability {
            id: id.into(),
            aliases: Vec::new(),
            summary: String::new(),
            severity,
            url: None,
        }
    }

    #[test]
    fn no_severity_filter_returns_all_unchanged() {
        let results = vec![
            make_audit_result(vec![
                make_vuln("V1", Severity::Low),
                make_vuln("V2", Severity::Critical),
            ]),
            make_audit_result(vec![make_vuln("V3", Severity::Medium)]),
        ];
        let filter = no_filter();
        let filtered = apply_severity_filter(results, &filter);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].vulnerabilities.len(), 2);
        assert_eq!(filtered[1].vulnerabilities.len(), 1);
    }

    #[test]
    fn severity_filter_high_removes_low_and_medium() {
        let results = vec![make_audit_result(vec![
            make_vuln("V-LOW", Severity::Low),
            make_vuln("V-MED", Severity::Medium),
            make_vuln("V-HIGH", Severity::High),
            make_vuln("V-CRIT", Severity::Critical),
        ])];
        let filter = Filter {
            severity: Some(Severity::High),
            ..no_filter()
        };
        let filtered = apply_severity_filter(results, &filter);
        assert_eq!(filtered.len(), 1);
        let vuln_ids: Vec<&str> = filtered[0]
            .vulnerabilities
            .iter()
            .map(|v| v.id.as_str())
            .collect();
        assert_eq!(vuln_ids, vec!["V-HIGH", "V-CRIT"]);
    }

    #[test]
    fn empty_results_returns_empty() {
        let filter = Filter {
            severity: Some(Severity::High),
            ..no_filter()
        };
        let filtered = apply_severity_filter(Vec::new(), &filter);
        assert!(filtered.is_empty());
    }

    #[test]
    fn result_with_no_vulnerabilities_passes_through() {
        let results = vec![make_audit_result(Vec::new())];
        let filter = Filter {
            severity: Some(Severity::Critical),
            ..no_filter()
        };
        let filtered = apply_severity_filter(results, &filter);
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].vulnerabilities.is_empty());
    }
}
