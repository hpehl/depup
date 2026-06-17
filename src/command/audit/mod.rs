//! The `audit` subcommand: checks dependencies for known vulnerabilities via OSV.dev.
//!
//! Reuses the check pipeline to discover dependencies and their versions,
//! then queries OSV.dev for known vulnerabilities. Supports the same
//! ecosystem/kind/include/exclude filters as check, plus severity filtering.

mod osv;

use anyhow::Result;
use clap::ArgMatches;
use tokio::time::Instant;

use crate::filter::Filter;
use crate::model::{AuditResult, CheckResult, CommandResult, DependencyKind, Severity};
use crate::output;
use crate::output::json::AuditJsonResult;
use crate::progress;

/// OSV audit has two phases: batch query + fetch vulnerability details.
const AUDIT_PHASES: u64 = 2;

/// Returns exit code 3 if critical/high vulnerabilities are found,
/// 2 if any vulnerabilities are found, 0 otherwise.
pub async fn audit(matches: &ArgMatches) -> Result<u8> {
    let setup = super::pipeline::CommandSetup::from_matches(matches);
    let instant = Instant::now();

    // Phase 1: Discover and check dependencies (reuse check pipeline for version info)
    let pipeline = crate::command::pipeline::resolve_versions(&setup.resolve_config()).await?;

    // Tool versions (Node.js, package managers) are excluded: they aren't registry
    // packages with OSV vulnerability advisories, so auditing them would be meaningless.
    let auditable: Vec<CheckResult> = pipeline
        .results
        .into_iter()
        .filter(|r| r.kind() != DependencyKind::Tool && setup.filter.matches(r))
        .collect();

    if auditable.is_empty() {
        crate::command::pipeline::print_empty(setup.json, "No dependencies to audit.");
        return Ok(0);
    }

    // Phase 2: Query OSV.dev for vulnerabilities
    let bar = progress::phase_bar("Auditing", AUDIT_PHASES, setup.json);

    let audit_results = osv::audit(&auditable, &bar).await?;
    bar.finish_with_message("done");

    // Phase 3: Apply severity filter
    let filtered: Vec<AuditResult> = apply_severity_filter(audit_results, &setup.filter);

    // Phase 4: Output
    if setup.json {
        let json_results: Vec<AuditJsonResult> =
            filtered.iter().map(AuditJsonResult::from).collect();
        output::print_json(&json_results);
    } else {
        println!();
        println!();
        output::print_table(&filtered, "No dependencies to show.", output::audit_summary);
        progress::done(instant);
    }

    let has_critical_or_high = filtered.iter().any(|r| {
        r.vulnerabilities
            .iter()
            .any(|v| matches!(v.severity, Severity::Critical | Severity::High))
    });
    if has_critical_or_high {
        Ok(3)
    } else if filtered.iter().any(|r| r.is_vulnerable()) {
        Ok(2)
    } else {
        Ok(0)
    }
}

fn apply_severity_filter(results: Vec<AuditResult>, filter: &Filter) -> Vec<AuditResult> {
    if filter.severity.is_none() && !filter.vulnerable {
        return results;
    }

    results
        .into_iter()
        .map(|r| filter_vulns_by_severity(r, filter))
        .filter(|(r, was_vulnerable)| should_include(r, *was_vulnerable, filter))
        .map(|(r, _)| r)
        .collect()
}

fn filter_vulns_by_severity(r: AuditResult, filter: &Filter) -> (AuditResult, bool) {
    let was_vulnerable = r.is_vulnerable();
    let filtered = AuditResult {
        vulnerabilities: r
            .vulnerabilities
            .into_iter()
            .filter(|v| filter.matches_severity(v.severity))
            .collect(),
        ..r
    };
    (filtered, was_vulnerable)
}

fn should_include(r: &AuditResult, was_vulnerable: bool, filter: &Filter) -> bool {
    if filter.vulnerable {
        r.is_vulnerable()
    } else {
        r.is_vulnerable() || !was_vulnerable
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Dependency, Ecosystem, Severity, Vulnerability};

    fn make_audit_result(vulns: Vec<Vulnerability>) -> AuditResult {
        AuditResult {
            dep: Dependency::new(
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

    #[test]
    fn no_severity_filter_returns_all_unchanged() {
        let results = vec![
            make_audit_result(vec![
                Vulnerability::test("V1", Severity::Low),
                Vulnerability::test("V2", Severity::Critical),
            ]),
            make_audit_result(vec![Vulnerability::test("V3", Severity::Medium)]),
        ];
        let filter = Filter::default();
        let filtered = apply_severity_filter(results, &filter);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].vulnerabilities.len(), 2);
        assert_eq!(filtered[1].vulnerabilities.len(), 1);
    }

    #[test]
    fn severity_filter_high_removes_low_and_medium() {
        let results = vec![make_audit_result(vec![
            Vulnerability::test("V-LOW", Severity::Low),
            Vulnerability::test("V-MED", Severity::Medium),
            Vulnerability::test("V-HIGH", Severity::High),
            Vulnerability::test("V-CRIT", Severity::Critical),
        ])];
        let filter = Filter {
            severity: Some(Severity::High),
            ..Filter::default()
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
            ..Filter::default()
        };
        let filtered = apply_severity_filter(Vec::new(), &filter);
        assert!(filtered.is_empty());
    }

    #[test]
    fn result_with_no_vulnerabilities_passes_through() {
        let results = vec![make_audit_result(Vec::new())];
        let filter = Filter {
            severity: Some(Severity::Critical),
            ..Filter::default()
        };
        let filtered = apply_severity_filter(results, &filter);
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].vulnerabilities.is_empty());
    }

    #[test]
    fn vulnerable_flag_hides_clean_dependencies() {
        let results = vec![
            make_audit_result(Vec::new()),
            make_audit_result(vec![Vulnerability::test("V1", Severity::High)]),
        ];
        let filter = Filter {
            vulnerable: true,
            ..Filter::default()
        };
        let filtered = apply_severity_filter(results, &filter);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].vulnerabilities[0].id, "V1");
    }

    #[test]
    fn vulnerable_flag_with_severity_filter() {
        let results = vec![
            make_audit_result(vec![Vulnerability::test("V-LOW", Severity::Low)]),
            make_audit_result(vec![Vulnerability::test("V-CRIT", Severity::Critical)]),
            make_audit_result(Vec::new()),
        ];
        let filter = Filter {
            vulnerable: true,
            severity: Some(Severity::High),
            ..Filter::default()
        };
        let filtered = apply_severity_filter(results, &filter);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].vulnerabilities[0].id, "V-CRIT");
    }

    #[test]
    fn severity_filter_drops_results_with_only_below_threshold_vulns() {
        let results = vec![
            make_audit_result(vec![Vulnerability::test("V-LOW", Severity::Low)]),
            make_audit_result(vec![Vulnerability::test("V-CRIT", Severity::Critical)]),
        ];
        let filter = Filter {
            severity: Some(Severity::High),
            ..Filter::default()
        };
        let filtered = apply_severity_filter(results, &filter);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].vulnerabilities[0].id, "V-CRIT");
    }
}
