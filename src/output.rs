//! Terminal and JSON output formatting.
//!
//! Results are grouped by ecosystem (Maven, npm) and then by kind
//! (Dependencies, Plugins, Tool Versions, Dev Dependencies). Each line
//! shows the artifact name, current version, source file, and status
//! with color-coded symbols.

use std::collections::BTreeSet;

use console::style;

use crate::dependency::{
    AuditResult, DependencyKind, Ecosystem, Severity, UpdateResult, VersionResult, VersionStatus,
};
use crate::json::{AuditJsonResult, JsonResult, UpdateJsonResult};

const ARTIFACT_WIDTH: usize = 40;
const VERSION_WIDTH: usize = 30;
const SOURCE_WIDTH: usize = 25;

fn kind_color(kind: DependencyKind) -> console::Style {
    match kind {
        DependencyKind::Dependency => console::Style::new().cyan(),
        DependencyKind::Plugin => console::Style::new().magenta(),
        DependencyKind::ToolVersion => console::Style::new().green(),
        DependencyKind::NpmDep | DependencyKind::NpmDevDep => console::Style::new().blue(),
    }
}

fn kind_symbol(kind: DependencyKind) -> &'static str {
    match kind {
        DependencyKind::Dependency => "\u{25cf}",
        DependencyKind::Plugin => "\u{25a0}",
        DependencyKind::ToolVersion => "\u{25b2}",
        DependencyKind::NpmDep | DependencyKind::NpmDevDep => "\u{25c6}",
    }
}

fn kind_group_label(kind: DependencyKind) -> &'static str {
    match kind {
        DependencyKind::Dependency => "Dependencies",
        DependencyKind::Plugin => "Plugins",
        DependencyKind::ToolVersion => "Tool Versions",
        DependencyKind::NpmDep => "Dependencies",
        DependencyKind::NpmDevDep => "Dev Dependencies",
    }
}

/// Prints a kind legend suffix like `  (● Dependency, ■ Plugin)` at the end of a summary line.
fn print_kind_legend(kinds: &[DependencyKind]) {
    let mut sorted = kinds.to_vec();
    sorted.sort();
    sorted.dedup();
    let legend: Vec<String> = sorted
        .iter()
        .map(|k| format!("{} {k}", kind_color(*k).apply_to(kind_symbol(*k))))
        .collect();
    println!("  ({})", legend.join(", "));
}

/// Prints results as a JSON array to stdout.
pub fn print_json(results: &[VersionResult]) {
    let json_results: Vec<JsonResult> = results.iter().map(JsonResult::from).collect();

    println!(
        "{}",
        serde_json::to_string_pretty(&json_results).unwrap_or_else(|_| "[]".to_string())
    );
}

fn print_ecosystem_header(ecosystem: Ecosystem) {
    let label = ecosystem.to_string();
    let line = "\u{2500}".repeat(3);
    println!(
        "{} {} {}",
        style(line.clone()).dim(),
        style(label).bold(),
        style(line).dim()
    );
}

/// Groups items by ecosystem, then by kind, printing section headers and
/// calling `print_item` for each entry. Extracts the shared grouping/sorting
/// logic used by both check and update output.
fn print_grouped<T>(
    items: &[T],
    get_ecosystem: impl Fn(&T) -> Ecosystem,
    get_kind: impl Fn(&T) -> DependencyKind,
    get_sort_key: impl Fn(&T) -> &str,
    print_item: impl Fn(&T),
) {
    let ecosystems: Vec<Ecosystem> = items
        .iter()
        .map(&get_ecosystem)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let multiple_ecosystems = ecosystems.len() > 1;
    for ecosystem in &ecosystems {
        let mut group: Vec<&T> = items
            .iter()
            .filter(|r| get_ecosystem(r) == *ecosystem)
            .collect();
        group.sort_by(|a, b| {
            get_kind(a)
                .cmp(&get_kind(b))
                .then_with(|| get_sort_key(a).cmp(get_sort_key(b)))
        });

        if multiple_ecosystems {
            print_ecosystem_header(*ecosystem);
        }

        let kinds: Vec<DependencyKind> = group
            .iter()
            .map(|r| get_kind(r))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let multiple_kinds = kinds.len() > 1;

        for kind in &kinds {
            if multiple_kinds {
                println!("  {}", style(kind_group_label(*kind)).dim().bold());
            }
            for item in group.iter().filter(|r| get_kind(r) == *kind) {
                print_item(item);
            }
        }
        println!();
    }
}

/// Prints a styled summary table to stdout, grouped by ecosystem and kind.
pub fn print_results(results: &[VersionResult]) {
    if results.is_empty() {
        println!("{}", style("No dependencies to show.").dim());
        return;
    }

    print_grouped(
        results,
        |r| r.ecosystem(),
        |r| r.kind(),
        |r| r.artifact(),
        print_result_line,
    );
    print_summary(results);
}

fn print_result_line(result: &VersionResult) {
    let artifact = truncate_middle_pad(result.artifact(), ARTIFACT_WIDTH);
    let styled_artifact = kind_color(result.kind()).apply_to(artifact);

    let version_label = format_version(result);
    let version = truncate_middle_pad(&version_label, VERSION_WIDTH);

    let source = truncate_middle_pad(result.source(), SOURCE_WIDTH);

    match &result.status {
        VersionStatus::UpToDate { .. } => {
            println!(
                "  {} {}  {}  {}  {}",
                style("\u{2713}").green().bold(),
                styled_artifact,
                style(version).white(),
                style(source).dim(),
                style("up-to-date").green()
            );
        }
        VersionStatus::Outdated { latest } => {
            println!(
                "  {} {}  {}  {}  {}",
                style("\u{2192}").yellow().bold(),
                styled_artifact,
                style(version).white(),
                style(source).dim(),
                style(format!("\u{2192} {latest}")).yellow()
            );
        }
        VersionStatus::Skipped => {
            println!(
                "  {} {}  {}  {}  {}",
                style("-").dim().bold(),
                styled_artifact,
                style(version).dim(),
                style(source).dim(),
                style("skipped").dim()
            );
        }
        VersionStatus::Error { message } => {
            println!(
                "  {} {}  {}  {}  {}",
                style("\u{2717}").red().bold(),
                styled_artifact,
                style(version).white(),
                style(source).dim(),
                style(message).red()
            );
        }
    }
}

/// Formats the version column, appending the property name in parentheses
/// for Maven managed dependencies (those backed by a `<properties>` entry).
fn format_version(result: &VersionResult) -> String {
    if result.current_version.is_empty() {
        return String::new();
    }
    match result.property() {
        Some(prop) => format!("{} ({})", result.current_version, prop),
        None => result.current_version.clone(),
    }
}

/// Truncates a string to `width` characters using middle-ellipsis, or right-pads if shorter.
fn truncate_middle_pad(s: &str, width: usize) -> String {
    let char_count = s.chars().count();
    if char_count > width {
        let ellipsis = "\u{2026}";
        let half = (width - 1) / 2;
        let remainder = width - 1 - half;
        let prefix: String = s.chars().take(half).collect();
        let suffix: String = s.chars().skip(char_count - remainder).collect();
        format!("{prefix}{ellipsis}{suffix}")
    } else {
        format!("{s:<width$}")
    }
}

/// Prints a one-line summary with counts and a legend of kind symbols.
fn print_summary(results: &[VersionResult]) {
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

/// Prints update results as a JSON array to stdout.
pub fn print_update_json(results: &[UpdateJsonResult]) {
    println!(
        "{}",
        serde_json::to_string_pretty(&results).unwrap_or_else(|_| "[]".to_string())
    );
}

/// Prints a styled update results table, grouped by ecosystem and kind.
pub fn print_update_results(results: &[UpdateResult]) {
    if results.is_empty() {
        return;
    }

    print_grouped(
        results,
        |r| r.ecosystem,
        |r| r.kind,
        |r| r.artifact.as_str(),
        print_update_line,
    );
    print_update_summary(results);
}

fn print_update_line(result: &UpdateResult) {
    let artifact = truncate_middle_pad(&result.artifact, ARTIFACT_WIDTH);
    let styled_artifact = kind_color(result.kind).apply_to(artifact);

    let version_label = format!("{} \u{2192} {}", result.old_version, result.new_version);
    let version = truncate_middle_pad(&version_label, VERSION_WIDTH);

    let source = truncate_middle_pad(&result.source, SOURCE_WIDTH);

    match &result.status {
        crate::dependency::UpdateStatus::Updated => {
            println!(
                "  {} {}  {}  {}  {}",
                style("\u{2713}").green().bold(),
                styled_artifact,
                style(version).white(),
                style(source).dim(),
                style("updated").green()
            );
        }
        crate::dependency::UpdateStatus::Error { message } => {
            println!(
                "  {} {}  {}  {}  {}",
                style("\u{2717}").red().bold(),
                styled_artifact,
                style(version).white(),
                style(source).dim(),
                style(message).red()
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Audit output
// ---------------------------------------------------------------------------

/// Prints audit results as a JSON array to stdout.
pub fn print_audit_json(results: &[AuditResult]) {
    let json_results: Vec<AuditJsonResult> = results.iter().map(AuditJsonResult::from).collect();
    println!(
        "{}",
        serde_json::to_string_pretty(&json_results).unwrap_or_else(|_| "[]".to_string())
    );
}

/// Prints a styled audit results table, grouped by ecosystem and kind.
pub fn print_audit_results(results: &[AuditResult]) {
    if results.is_empty() {
        println!("{}", style("No dependencies to show.").dim());
        return;
    }

    print_grouped(
        results,
        |r| r.ecosystem,
        |r| r.kind,
        |r| r.artifact.as_str(),
        print_audit_line,
    );
    print_audit_summary(results);
}

fn severity_style(severity: Severity) -> console::Style {
    match severity {
        Severity::Critical => console::Style::new().red().bold(),
        Severity::High => console::Style::new().red(),
        Severity::Medium => console::Style::new().yellow(),
        Severity::Low => console::Style::new().dim(),
        Severity::Unknown => console::Style::new().dim(),
    }
}

fn print_audit_line(result: &AuditResult) {
    let artifact = truncate_middle_pad(&result.artifact, ARTIFACT_WIDTH);
    let styled_artifact = kind_color(result.kind).apply_to(artifact);

    let version = truncate_middle_pad(&result.version, VERSION_WIDTH);
    let source = truncate_middle_pad(&result.source, SOURCE_WIDTH);

    if result.vulnerabilities.is_empty() {
        println!(
            "  {} {}  {}  {}  {}",
            style("\u{2713}").green().bold(),
            styled_artifact,
            style(version).white(),
            style(source).dim(),
            style("no vulnerabilities").green()
        );
    } else {
        let count = result.vulnerabilities.len();
        let max_sev = result.max_severity();
        let label = if count == 1 {
            "vulnerability".to_string()
        } else {
            "vulnerabilities".to_string()
        };
        println!(
            "  {} {}  {}  {}  {}",
            style("\u{2717}").red().bold(),
            styled_artifact,
            style(version).white(),
            style(source).dim(),
            severity_style(max_sev).apply_to(format!("{count} {label}")),
        );
        for vuln in &result.vulnerabilities {
            let id_and_aliases = if vuln.aliases.is_empty() {
                vuln.id.clone()
            } else {
                format!("{} ({})", vuln.id, vuln.aliases.join(", "))
            };
            let summary = if vuln.summary.is_empty() {
                String::new()
            } else {
                let truncated = truncate_middle_pad(&vuln.summary, 60);
                format!(" {truncated}")
            };
            println!(
                "      {} {}{}",
                severity_style(vuln.severity).apply_to(format!("[{}]", vuln.severity)),
                style(id_and_aliases).dim(),
                style(summary).dim(),
            );
        }
    }
}

fn print_audit_summary(results: &[AuditResult]) {
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

    let kinds: Vec<DependencyKind> = results.iter().map(|r| r.kind).collect();
    print_kind_legend(&kinds);
}

fn print_update_summary(results: &[UpdateResult]) {
    let total = results.len();
    let errors = results.iter().filter(|r| r.is_error()).count();

    print!("{total} updated");
    if errors > 0 {
        print!(", {}", style(format!("{errors} errors")).red());
    }

    let kinds: Vec<DependencyKind> = results.iter().map(|r| r.kind).collect();
    print_kind_legend(&kinds);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dependency::Dependency;

    #[test]
    fn print_table_groups_by_ecosystem() {
        let results = vec![
            VersionResult::checked(
                Dependency::new(
                    Ecosystem::Maven,
                    DependencyKind::Dependency,
                    "org.junit.jupiter:junit-jupiter".to_string(),
                    Some("version.junit".to_string()),
                    "pom.xml".to_string(),
                ),
                "5.10.0".to_string(),
                "5.12.0".to_string(),
                true,
            ),
            VersionResult::checked(
                Dependency::new(
                    Ecosystem::Npm,
                    DependencyKind::NpmDep,
                    "react".to_string(),
                    None,
                    "package.json".to_string(),
                ),
                "18.0.0".to_string(),
                "19.0.0".to_string(),
                true,
            ),
        ];

        let ecosystems: Vec<Ecosystem> = results
            .iter()
            .map(|r| r.ecosystem())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        assert_eq!(ecosystems, vec![Ecosystem::Maven, Ecosystem::Npm]);
    }

    #[test]
    fn format_version_with_property() {
        let r = VersionResult::checked(
            Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                "org.junit.jupiter:junit-jupiter".to_string(),
                Some("version.junit".to_string()),
                String::new(),
            ),
            "5.10.0".to_string(),
            "5.12.0".to_string(),
            true,
        );
        assert_eq!(format_version(&r), "5.10.0 (version.junit)");
    }

    #[test]
    fn format_version_plain() {
        let r = VersionResult::checked(
            Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                "com.google.guava:guava".to_string(),
                None,
                String::new(),
            ),
            "33.0.0-jre".to_string(),
            "33.4.0-jre".to_string(),
            true,
        );
        assert_eq!(format_version(&r), "33.0.0-jre");
    }

    #[test]
    fn format_version_npm() {
        let r = VersionResult::checked(
            Dependency::new(
                Ecosystem::Npm,
                DependencyKind::NpmDep,
                "react".to_string(),
                None,
                String::new(),
            ),
            "18.2.0".to_string(),
            "19.0.0".to_string(),
            true,
        );
        assert_eq!(format_version(&r), "18.2.0");
    }

    #[test]
    fn format_version_empty() {
        let r = VersionResult::error(
            Dependency::new(
                Ecosystem::Npm,
                DependencyKind::NpmDep,
                "my-app".to_string(),
                None,
                String::new(),
            ),
            String::new(),
            "pnpm not found".to_string(),
        );
        assert_eq!(format_version(&r), "");
    }

    #[test]
    fn truncate_short_string_pads() {
        let result = truncate_middle_pad("hello", 10);
        assert_eq!(result, "hello     ");
        assert_eq!(result.len(), 10);
    }

    #[test]
    fn truncate_exact_width_no_change() {
        let result = truncate_middle_pad("abcde", 5);
        assert_eq!(result, "abcde");
    }

    #[test]
    fn truncate_long_string_inserts_ellipsis() {
        let result = truncate_middle_pad("abcdefghij", 7);
        assert!(result.contains('\u{2026}'));
        assert_eq!(result.chars().count(), 7);
        assert!(result.starts_with("abc"));
        assert!(result.ends_with("hij"));
    }

    #[test]
    fn truncate_multibyte_does_not_panic() {
        let result = truncate_middle_pad("ä\u{f6}ü\u{e9}\u{e8}\u{ea}\u{eb}\u{e0}", 5);
        assert_eq!(result.chars().count(), 5);
    }
}
