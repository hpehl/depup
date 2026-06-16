use console::style;

use crate::model::{
    AuditResult, CommandResult, DependencyKind, Severity, UpdateResult, VersionResult,
};

use super::format::print_kind_legend;

pub fn check_summary(results: &[VersionResult]) {
    let total = results.len();
    let outdated = results.iter().filter(|r| r.is_outdated()).count();
    let skipped = results.iter().filter(|r| r.is_skipped()).count();
    let errors = results
        .iter()
        .filter(|r| r.error_message().is_some())
        .count();
    let current = total - outdated - skipped - errors;

    print!("{total} checked: ");
    print!("{}", style(format!("{current} current")).green());
    if outdated > 0 {
        print!(", {}", style(format!("{outdated} outdated")).yellow());
    }
    if skipped > 0 {
        print!(", {}", style(format!("{skipped} skipped")).dim());
    }
    if errors > 0 {
        print!(", {}", style(format!("{errors} errors")).red());
    }

    let kinds: Vec<DependencyKind> = results.iter().map(|r| r.kind()).collect();
    print_kind_legend(&kinds);
}

pub fn update_summary(results: &[UpdateResult]) {
    let total = results.len();
    let errors = results.iter().filter(|r| r.is_error()).count();

    print!("{total} updated");
    if errors > 0 {
        print!(", {}", style(format!("{errors} errors")).red());
    }

    let kinds: Vec<DependencyKind> = results.iter().map(|r| r.kind()).collect();
    print_kind_legend(&kinds);
}

pub fn audit_summary(results: &[AuditResult]) {
    let total = results.len();
    let vulnerable = results.iter().filter(|r| r.is_vulnerable()).count();
    let clean = total - vulnerable;

    print!("{total} audited: ");
    print!("{}", style(format!("{clean} clean")).green());
    if vulnerable > 0 {
        print!(", {}", style(format!("{vulnerable} vulnerable")).red());

        let all_vulns: Vec<Severity> = results
            .iter()
            .flat_map(|r| r.vulnerabilities.iter().map(|v| v.severity))
            .collect();
        let critical = all_vulns
            .iter()
            .filter(|s| **s == Severity::Critical)
            .count();
        let high = all_vulns.iter().filter(|s| **s == Severity::High).count();
        let medium = all_vulns.iter().filter(|s| **s == Severity::Medium).count();
        let low = all_vulns.iter().filter(|s| **s == Severity::Low).count();

        let mut parts = Vec::new();
        if critical > 0 {
            parts.push(format!("{critical} critical"));
        }
        if high > 0 {
            parts.push(format!("{high} high"));
        }
        if medium > 0 {
            parts.push(format!("{medium} medium"));
        }
        if low > 0 {
            parts.push(format!("{low} low"));
        }
        if !parts.is_empty() {
            print!(" ({})", parts.join(", "));
        }
    }

    let kinds: Vec<DependencyKind> = results.iter().map(|r| r.kind()).collect();
    print_kind_legend(&kinds);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Dependency, Ecosystem, Vulnerability};

    fn make_check_result(artifact: &str, kind: DependencyKind, outdated: bool) -> VersionResult {
        VersionResult::checked(
            Dependency::new(Ecosystem::Maven, kind, artifact.into(), None, String::new()),
            "1.0.0".into(),
            "2.0.0".into(),
            outdated,
        )
    }

    fn make_update_result(artifact: &str, is_error: bool) -> UpdateResult {
        let check = VersionResult::checked(
            Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                artifact.into(),
                None,
                String::new(),
            ),
            "1.0.0".into(),
            "2.0.0".into(),
            true,
        );
        if is_error {
            UpdateResult::error(&check, "write failed".into())
        } else {
            UpdateResult::updated(&check, "2.0.0".into())
        }
    }

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
    fn check_summary_with_mixed_results_does_not_panic() {
        let results = vec![
            make_check_result("g:a", DependencyKind::Dependency, false),
            make_check_result("g:b", DependencyKind::Dependency, true),
            VersionResult::error(
                Dependency::new(
                    Ecosystem::Maven,
                    DependencyKind::Plugin,
                    "g:c".into(),
                    None,
                    String::new(),
                ),
                "1.0".into(),
                "timeout".into(),
            ),
        ];
        check_summary(&results);
    }

    #[test]
    fn update_summary_with_errors_does_not_panic() {
        let results = vec![
            make_update_result("g:a", false),
            make_update_result("g:b", true),
        ];
        update_summary(&results);
    }

    #[test]
    fn audit_summary_with_vulnerabilities_does_not_panic() {
        let results = vec![
            make_audit_result(vec![
                make_vuln("CVE-1", Severity::Critical),
                make_vuln("CVE-2", Severity::Low),
            ]),
            make_audit_result(Vec::new()),
        ];
        audit_summary(&results);
    }

    #[test]
    fn audit_summary_all_clean_does_not_panic() {
        let results = vec![make_audit_result(Vec::new()), make_audit_result(Vec::new())];
        audit_summary(&results);
    }

    #[test]
    fn check_summary_empty_does_not_panic() {
        check_summary(&[]);
    }

    #[test]
    fn update_summary_empty_does_not_panic() {
        update_summary(&[]);
    }

    #[test]
    fn audit_summary_empty_does_not_panic() {
        audit_summary(&[]);
    }
}
