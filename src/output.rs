use std::collections::BTreeSet;

use console::style;

use crate::json::JsonResult;
use crate::progress::truncate_middle_pad;
use crate::registry::{CheckResult, CheckerKind, Ecosystem, Outcome};

const ARTIFACT_WIDTH: usize = 40;
const VERSION_WIDTH: usize = 30;
const SOURCE_WIDTH: usize = 25;

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

pub fn print_results(results: &[CheckResult]) {
    let ecosystems: Vec<Ecosystem> = results
        .iter()
        .map(|r| r.ecosystem)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let multiple_ecosystems = ecosystems.len() > 1;
    for ecosystem in &ecosystems {
        let mut group: Vec<&CheckResult> = results
            .iter()
            .filter(|r| r.ecosystem == *ecosystem)
            .collect();
        group.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| {
            let a_name = a.artifact.as_deref().unwrap_or(&a.property_name);
            let b_name = b.artifact.as_deref().unwrap_or(&b.property_name);
            a_name.cmp(b_name)
        }));

        if multiple_ecosystems {
            print_ecosystem_header(*ecosystem);
        }

        let kinds: Vec<CheckerKind> = group
            .iter()
            .map(|r| r.kind)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let multiple_kinds = kinds.len() > 1;

        for kind in &kinds {
            if multiple_kinds {
                println!("  {}", style(kind.group_label()).dim().bold());
            }
            for result in group.iter().filter(|r| r.kind == *kind) {
                print_result_line(result);
            }
        }
        println!();
    }
    print_summary(results);
}

fn print_result_line(result: &CheckResult) {
    let artifact_name = result
        .artifact
        .as_deref()
        .unwrap_or(&result.property_name);
    let artifact = truncate_middle_pad(artifact_name, ARTIFACT_WIDTH);
    let styled_artifact = result.kind.color().apply_to(artifact);

    let version_label = format_version(result);
    let version = truncate_middle_pad(&version_label, VERSION_WIDTH);

    let source = truncate_middle_pad(&result.source, SOURCE_WIDTH);

    match result.outcome() {
        Outcome::UpToDate => {
            println!(
                "  {} {}  {}  {}  {}",
                style("\u{2713}").green().bold(),
                styled_artifact,
                style(version).white(),
                style(source).dim(),
                style("up-to-date").green()
            );
        }
        Outcome::Outdated { latest } => {
            println!(
                "  {} {}  {}  {}  {}",
                style("\u{2192}").yellow().bold(),
                styled_artifact,
                style(version).white(),
                style(source).dim(),
                style(format!("\u{2192} {latest}")).yellow()
            );
        }
        Outcome::Skipped => {
            println!(
                "  {} {}  {}  {}  {}",
                style("-").dim().bold(),
                styled_artifact,
                style(version).dim(),
                style(source).dim(),
                style("skipped").dim()
            );
        }
        Outcome::Error { message } => {
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

fn format_version(result: &CheckResult) -> String {
    if result.current_version.is_empty() {
        return String::new();
    }
    let artifact_name = result
        .artifact
        .as_deref()
        .unwrap_or(&result.property_name);
    if result.property_name != artifact_name && !result.property_name.contains(':') {
        format!("{} ({})", result.current_version, result.property_name)
    } else {
        result.current_version.clone()
    }
}

fn print_summary(results: &[CheckResult]) {
    let total = results.len();
    let outdated = results.iter().filter(|r| r.outdated).count();
    let skipped = results.iter().filter(|r| r.skipped).count();
    let errors = results.iter().filter(|r| r.error.is_some()).count();
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

    let mut kinds: Vec<CheckerKind> = results.iter().map(|r| r.kind).collect();
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

    #[test]
    fn print_table_groups_by_ecosystem() {
        let results = vec![
            CheckResult {
                ecosystem: Ecosystem::Maven,
                property_name: "version.junit".to_string(),
                current_version: "5.10.0".to_string(),
                latest_version: Some("5.12.0".to_string()),
                outdated: true,
                skipped: false,
                error: None,
                artifact: Some("org.junit.jupiter:junit-jupiter".to_string()),
                source: "pom.xml".to_string(),
                kind: CheckerKind::Dependency,
            },
            CheckResult {
                ecosystem: Ecosystem::Pnpm,
                property_name: "react".to_string(),
                current_version: "18.0.0".to_string(),
                latest_version: Some("19.0.0".to_string()),
                outdated: true,
                skipped: false,
                error: None,
                artifact: Some("react".to_string()),
                source: "package.json".to_string(),
                kind: CheckerKind::Pnpm,
            },
        ];

        let ecosystems: Vec<Ecosystem> = results
            .iter()
            .map(|r| r.ecosystem)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        assert_eq!(ecosystems, vec![Ecosystem::Maven, Ecosystem::Pnpm]);
    }

    #[test]
    fn status_label_error() {
        let r = CheckResult::error(
            Ecosystem::Maven,
            CheckerKind::Dependency,
            "p".to_string(),
            "1.0".to_string(),
            None,
            "fail".to_string(),
        );
        assert_eq!(JsonResult::from(&r).status, "error");
    }

    #[test]
    fn status_label_outdated() {
        let r = CheckResult::checked(
            Ecosystem::Maven,
            CheckerKind::Dependency,
            "p".to_string(),
            "1.0".to_string(),
            "2.0".to_string(),
            true,
            None,
        );
        assert_eq!(JsonResult::from(&r).status, "outdated");
    }

    #[test]
    fn status_label_up_to_date() {
        let r = CheckResult::checked(
            Ecosystem::Maven,
            CheckerKind::Dependency,
            "p".to_string(),
            "1.0".to_string(),
            "1.0".to_string(),
            false,
            None,
        );
        assert_eq!(JsonResult::from(&r).status, "up-to-date");
    }

    #[test]
    fn json_output_structure() {
        let r = CheckResult::checked(
            Ecosystem::Maven,
            CheckerKind::Dependency,
            "version.junit".to_string(),
            "5.10.0".to_string(),
            "5.12.0".to_string(),
            true,
            Some("org.junit.jupiter:junit-jupiter".to_string()),
        );
        let json_result = JsonResult::from(&r);
        assert_eq!(json_result.status, "outdated");
        assert_eq!(json_result.kind, "dependency");
        assert_eq!(
            json_result.artifact.as_deref(),
            Some("org.junit.jupiter:junit-jupiter")
        );
    }

    #[test]
    fn json_output_includes_ecosystem() {
        let r = CheckResult::checked(
            Ecosystem::Maven,
            CheckerKind::Dependency,
            "version.junit".to_string(),
            "5.10.0".to_string(),
            "5.12.0".to_string(),
            true,
            Some("org.junit.jupiter:junit-jupiter".to_string()),
        );
        let json_result = JsonResult::from(&r);
        assert_eq!(json_result.ecosystem, "maven");
    }

    #[test]
    fn status_label_skipped() {
        let r = CheckResult::skipped(
            Ecosystem::Maven,
            CheckerKind::Dependency,
            "p".to_string(),
            "1.0-alpha".to_string(),
            None,
        );
        assert_eq!(JsonResult::from(&r).status, "skipped");
    }

    #[test]
    fn format_version_with_property() {
        let r = CheckResult::checked(
            Ecosystem::Maven,
            CheckerKind::Dependency,
            "version.junit".to_string(),
            "5.10.0".to_string(),
            "5.12.0".to_string(),
            true,
            Some("org.junit.jupiter:junit-jupiter".to_string()),
        );
        assert_eq!(format_version(&r), "5.10.0 (version.junit)");
    }

    #[test]
    fn format_version_plain() {
        let r = CheckResult::checked(
            Ecosystem::Maven,
            CheckerKind::Dependency,
            "com.google.guava:guava".to_string(),
            "33.0.0-jre".to_string(),
            "33.4.0-jre".to_string(),
            true,
            Some("com.google.guava:guava".to_string()),
        );
        assert_eq!(format_version(&r), "33.0.0-jre");
    }

    #[test]
    fn format_version_pnpm() {
        let r = CheckResult::checked(
            Ecosystem::Pnpm,
            CheckerKind::Pnpm,
            "react".to_string(),
            "18.2.0".to_string(),
            "19.0.0".to_string(),
            true,
            Some("react".to_string()),
        );
        assert_eq!(format_version(&r), "18.2.0");
    }

    #[test]
    fn format_version_empty() {
        let r = CheckResult::error(
            Ecosystem::Pnpm,
            CheckerKind::Pnpm,
            "my-app".to_string(),
            String::new(),
            None,
            "pnpm not found".to_string(),
        );
        assert_eq!(format_version(&r), "");
    }
}
