//! Terminal and JSON output formatting.
//!
//! All subcommands share the same output functions:
//! - [`print_table`] groups results by ecosystem/kind and prints styled lines.
//! - [`print_json`] serializes any `Serialize` value as pretty JSON.
//!
//! Each result type implements [`OutputLine`] to provide its version label
//! and styled status column, while the shared columns (artifact, source)
//! are formatted uniformly via the [`DependencyInfo`] trait.

use std::collections::BTreeSet;

use console::style;
use serde::Serialize;

use crate::dependency::{
    AuditResult, DependencyInfo, DependencyKind, Ecosystem, Severity, UpdateResult, VersionResult,
    VersionStatus,
};

const ARTIFACT_WIDTH: usize = 40;
const VERSION_WIDTH: usize = 30;
const SOURCE_WIDTH: usize = 25;

// ---------------------------------------------------------------------------
// OutputLine trait — subcommand-specific rendering
// ---------------------------------------------------------------------------

/// Subcommand-specific line rendering. Each result type provides its own
/// version label and status output while sharing the common column layout.
pub trait OutputLine: DependencyInfo {
    fn version_label(&self) -> String;
    fn print_line(&self);
}

impl OutputLine for VersionResult {
    fn version_label(&self) -> String {
        format_version_with_property(&self.current_version, self.property())
    }

    fn print_line(&self) {
        let (styled_artifact, version, source) = format_columns(self);
        match &self.status {
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
}

impl OutputLine for UpdateResult {
    fn version_label(&self) -> String {
        let old = format_version_with_property(&self.old_version, self.property());
        format!("{old} \u{2192} {}", self.new_version)
    }

    fn print_line(&self) {
        let (styled_artifact, version, source) = format_columns(self);
        match &self.status {
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
}

impl OutputLine for AuditResult {
    fn version_label(&self) -> String {
        format_version_with_property(&self.version, self.property())
    }

    fn print_line(&self) {
        let (styled_artifact, version, source) = format_columns(self);
        if self.vulnerabilities.is_empty() {
            println!(
                "  {} {}  {}  {}  {}",
                style("\u{2713}").green().bold(),
                styled_artifact,
                style(version).white(),
                style(source).dim(),
                style("no vulnerabilities").green()
            );
        } else {
            let count = self.vulnerabilities.len();
            let max_sev = self.max_severity();
            let label = if count == 1 {
                "vulnerability"
            } else {
                "vulnerabilities"
            };
            println!(
                "  {} {}  {}  {}  {}",
                style("\u{2717}").red().bold(),
                styled_artifact,
                style(version).white(),
                style(source).dim(),
                severity_style(max_sev).apply_to(format!("{count} {label}")),
            );
            for vuln in &self.vulnerabilities {
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
}

// ---------------------------------------------------------------------------
// Public API — two functions for all subcommands
// ---------------------------------------------------------------------------

/// Prints a styled table grouped by ecosystem and kind, with a summary line.
///
/// The `empty_msg` is shown when `results` is empty (pass `""` to print nothing).
/// The `summary` closure prints subcommand-specific summary statistics.
pub fn print_table<T: OutputLine>(results: &[T], empty_msg: &str, summary: impl Fn(&[T])) {
    if results.is_empty() {
        if !empty_msg.is_empty() {
            println!("{}", style(empty_msg).dim());
        }
        return;
    }

    print_grouped(results);
    summary(results);
}

/// Prints any serializable value as pretty JSON.
pub fn print_json(items: &impl Serialize) {
    println!(
        "{}",
        serde_json::to_string_pretty(items).unwrap_or_else(|_| "[]".to_string())
    );
}

// ---------------------------------------------------------------------------
// Summary helpers — called by subcommands via `print_table`
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Shared formatting internals
// ---------------------------------------------------------------------------

fn format_columns(item: &impl OutputLine) -> (console::StyledObject<String>, String, String) {
    let artifact = truncate_middle_pad(item.artifact(), ARTIFACT_WIDTH);
    let styled_artifact = kind_color(item.kind()).apply_to(artifact);
    let version = truncate_middle_pad(&item.version_label(), VERSION_WIDTH);
    let source = truncate_middle_pad(item.source(), SOURCE_WIDTH);
    (styled_artifact, version, source)
}

fn format_version_with_property(version: &str, property: Option<&str>) -> String {
    if version.is_empty() {
        return String::new();
    }
    match property {
        Some(prop) => format!("{version} ({prop})"),
        None => version.to_string(),
    }
}

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

fn severity_style(severity: Severity) -> console::Style {
    match severity {
        Severity::Critical => console::Style::new().red().bold(),
        Severity::High => console::Style::new().red(),
        Severity::Medium => console::Style::new().yellow(),
        Severity::Low => console::Style::new().dim(),
        Severity::Unknown => console::Style::new().dim(),
    }
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

fn print_grouped<T: OutputLine>(items: &[T]) {
    let ecosystems: Vec<Ecosystem> = items
        .iter()
        .map(|r| r.ecosystem())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let multiple_ecosystems = ecosystems.len() > 1;
    for ecosystem in &ecosystems {
        let mut group: Vec<&T> = items
            .iter()
            .filter(|r| r.ecosystem() == *ecosystem)
            .collect();
        group.sort_by(|a, b| {
            a.kind()
                .cmp(&b.kind())
                .then_with(|| a.artifact().cmp(b.artifact()))
        });

        if multiple_ecosystems {
            print_ecosystem_header(*ecosystem);
        }

        let kinds: Vec<DependencyKind> = group
            .iter()
            .map(|r| r.kind())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let multiple_kinds = kinds.len() > 1;

        for kind in &kinds {
            if multiple_kinds {
                println!("  {}", style(kind_group_label(*kind)).dim().bold());
            }
            for item in group.iter().filter(|r| r.kind() == *kind) {
                item.print_line();
            }
        }
        println!();
    }
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
    fn format_version_with_prop() {
        assert_eq!(
            format_version_with_property("5.10.0", Some("version.junit")),
            "5.10.0 (version.junit)"
        );
    }

    #[test]
    fn format_version_plain() {
        assert_eq!(
            format_version_with_property("33.0.0-jre", None),
            "33.0.0-jre"
        );
    }

    #[test]
    fn format_version_empty() {
        assert_eq!(format_version_with_property("", None), "");
        assert_eq!(format_version_with_property("", Some("prop")), "");
    }

    #[test]
    fn version_label_check_with_property() {
        let r = VersionResult::checked(
            Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                "g:a".into(),
                Some("version.junit".into()),
                String::new(),
            ),
            "5.10.0".into(),
            "5.12.0".into(),
            true,
        );
        assert_eq!(r.version_label(), "5.10.0 (version.junit)");
    }

    #[test]
    fn version_label_update_with_property() {
        let check = VersionResult::checked(
            Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                "g:a".into(),
                Some("version.junit".into()),
                String::new(),
            ),
            "5.10.0".into(),
            "5.12.0".into(),
            true,
        );
        let update = UpdateResult::updated(&check, "5.12.0".into());
        assert_eq!(
            update.version_label(),
            "5.10.0 (version.junit) \u{2192} 5.12.0"
        );
    }

    #[test]
    fn version_label_audit_with_property() {
        let r = AuditResult {
            id: Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                "g:a".into(),
                Some("version.junit".into()),
                String::new(),
            ),
            version: "5.10.0".into(),
            vulnerabilities: Vec::new(),
        };
        assert_eq!(r.version_label(), "5.10.0 (version.junit)");
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
