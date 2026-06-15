//! Terminal and JSON output formatting.
//!
//! Results are grouped by ecosystem (Maven, npm) and then by kind
//! (Dependencies, Plugins, Tool Versions, Dev Dependencies). Each line
//! shows the artifact name, current version, source file, and status
//! with color-coded symbols.

use std::collections::BTreeSet;

use console::style;

use crate::json::JsonResult;
use crate::registry::{CheckResult, CheckStatus, CheckerKind, Ecosystem};

const ARTIFACT_WIDTH: usize = 40;
const VERSION_WIDTH: usize = 30;
const SOURCE_WIDTH: usize = 25;

/// Prints results as a JSON array to stdout.
pub fn print_json(results: &[CheckResult]) {
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

/// Prints a styled summary table to stdout, grouped by ecosystem and kind.
pub fn print_results(results: &[CheckResult]) {
    if results.is_empty() {
        println!("{}", style("No dependencies to show.").dim());
        return;
    }

    let ecosystems: Vec<Ecosystem> = results
        .iter()
        .map(|r| r.ecosystem())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let multiple_ecosystems = ecosystems.len() > 1;
    for ecosystem in &ecosystems {
        let mut group: Vec<&CheckResult> = results
            .iter()
            .filter(|r| r.ecosystem() == *ecosystem)
            .collect();
        group.sort_by(|a, b| {
            a.kind().cmp(&b.kind()).then_with(|| {
                let a_name = a.artifact().unwrap_or(a.property_name());
                let b_name = b.artifact().unwrap_or(b.property_name());
                a_name.cmp(b_name)
            })
        });

        if multiple_ecosystems {
            print_ecosystem_header(*ecosystem);
        }

        let kinds: Vec<CheckerKind> = group
            .iter()
            .map(|r| r.kind())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let multiple_kinds = kinds.len() > 1;

        for kind in &kinds {
            if multiple_kinds {
                println!("  {}", style(kind.group_label()).dim().bold());
            }
            for result in group.iter().filter(|r| r.kind() == *kind) {
                print_result_line(result);
            }
        }
        println!();
    }
    print_summary(results);
}

fn print_result_line(result: &CheckResult) {
    let artifact_name = result.artifact().unwrap_or(result.property_name());
    let artifact = truncate_middle_pad(artifact_name, ARTIFACT_WIDTH);
    let styled_artifact = result.kind().color().apply_to(artifact);

    let version_label = format_version(result);
    let version = truncate_middle_pad(&version_label, VERSION_WIDTH);

    let source = truncate_middle_pad(result.source(), SOURCE_WIDTH);

    match &result.status {
        CheckStatus::UpToDate { .. } => {
            println!(
                "  {} {}  {}  {}  {}",
                style("\u{2713}").green().bold(),
                styled_artifact,
                style(version).white(),
                style(source).dim(),
                style("up-to-date").green()
            );
        }
        CheckStatus::Outdated { latest } => {
            println!(
                "  {} {}  {}  {}  {}",
                style("\u{2192}").yellow().bold(),
                styled_artifact,
                style(version).white(),
                style(source).dim(),
                style(format!("\u{2192} {latest}")).yellow()
            );
        }
        CheckStatus::Skipped => {
            println!(
                "  {} {}  {}  {}  {}",
                style("-").dim().bold(),
                styled_artifact,
                style(version).dim(),
                style(source).dim(),
                style("skipped").dim()
            );
        }
        CheckStatus::Error { message } => {
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
/// when the artifact name differs from the property name (Maven managed dependencies).
fn format_version(result: &CheckResult) -> String {
    if result.current_version.is_empty() {
        return String::new();
    }
    let artifact_name = result.artifact().unwrap_or(result.property_name());
    if result.property_name() != artifact_name && !result.property_name().contains(':') {
        format!("{} ({})", result.current_version, result.property_name())
    } else {
        result.current_version.clone()
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
fn print_summary(results: &[CheckResult]) {
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

    let mut kinds: Vec<CheckerKind> = results.iter().map(|r| r.kind()).collect();
    kinds.sort();
    kinds.dedup();
    let legend: Vec<String> = kinds
        .iter()
        .map(|k| format!("{} {k}", k.color().apply_to(k.symbol())))
        .collect();
    println!("  ({})", legend.join(", "));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::CheckId;

    #[test]
    fn print_table_groups_by_ecosystem() {
        let results = vec![
            CheckResult::checked(
                CheckId::new(
                    Ecosystem::Maven,
                    CheckerKind::Dependency,
                    "version.junit".to_string(),
                    Some("org.junit.jupiter:junit-jupiter".to_string()),
                    "pom.xml".to_string(),
                ),
                "5.10.0".to_string(),
                "5.12.0".to_string(),
                true,
            ),
            CheckResult::checked(
                CheckId::new(
                    Ecosystem::Npm,
                    CheckerKind::NpmDep,
                    "react".to_string(),
                    Some("react".to_string()),
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
        let r = CheckResult::checked(
            CheckId::new(
                Ecosystem::Maven,
                CheckerKind::Dependency,
                "version.junit".to_string(),
                Some("org.junit.jupiter:junit-jupiter".to_string()),
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
        let r = CheckResult::checked(
            CheckId::new(
                Ecosystem::Maven,
                CheckerKind::Dependency,
                "com.google.guava:guava".to_string(),
                Some("com.google.guava:guava".to_string()),
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
        let r = CheckResult::checked(
            CheckId::new(
                Ecosystem::Npm,
                CheckerKind::NpmDep,
                "react".to_string(),
                Some("react".to_string()),
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
        let r = CheckResult::error(
            CheckId::new(
                Ecosystem::Npm,
                CheckerKind::NpmDep,
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
