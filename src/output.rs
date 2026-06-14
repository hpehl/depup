use std::collections::BTreeSet;

use console::style;

use crate::json::JsonResult;
use crate::registry::{CheckResult, CheckerKind, Ecosystem};

pub fn print_json(results: &[CheckResult]) {
    let json_results: Vec<JsonResult> = results
        .iter()
        .map(|r| {
            let status = status_label(r);
            JsonResult {
                ecosystem: r.ecosystem.to_string().to_lowercase(),
                property: r.property_name.clone(),
                current: r.current_version.clone(),
                latest: r.latest_version.clone(),
                status: status.to_string(),
                kind: r.kind.to_string().to_lowercase(),
                error: r.error.clone(),
                artifact: r.artifact.clone(),
            }
        })
        .collect();

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

    let multiple = ecosystems.len() > 1;
    for ecosystem in &ecosystems {
        let group: Vec<&CheckResult> = results
            .iter()
            .filter(|r| r.ecosystem == *ecosystem)
            .collect();
        if multiple {
            print_ecosystem_header(*ecosystem);
        }
        print_summary(&group);
    }
}

fn print_summary(results: &[&CheckResult]) {
    let total = results.len();
    let outdated = results.iter().filter(|r| r.outdated).count();
    let skipped = results.iter().filter(|r| r.skipped).count();
    let errors = results.iter().filter(|r| r.error.is_some()).count();
    let current = total - outdated - skipped - errors;

    print!("{total} properties checked: ");
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

fn status_label(result: &CheckResult) -> &'static str {
    if result.error.is_some() {
        "error"
    } else if result.skipped {
        "skipped"
    } else if result.outdated {
        "outdated"
    } else {
        "up-to-date"
    }
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
        let r = CheckResult {
            ecosystem: Ecosystem::Maven,
            property_name: "p".to_string(),
            current_version: "1.0".to_string(),
            latest_version: None,
            outdated: false,
            skipped: false,
            error: Some("fail".to_string()),
            artifact: None,
            kind: CheckerKind::Dependency,
        };
        assert_eq!(status_label(&r), "error");
    }

    #[test]
    fn status_label_outdated() {
        let r = CheckResult {
            ecosystem: Ecosystem::Maven,
            property_name: "p".to_string(),
            current_version: "1.0".to_string(),
            latest_version: Some("2.0".to_string()),
            outdated: true,
            skipped: false,
            error: None,
            artifact: None,
            kind: CheckerKind::Dependency,
        };
        assert_eq!(status_label(&r), "outdated");
    }

    #[test]
    fn status_label_up_to_date() {
        let r = CheckResult {
            ecosystem: Ecosystem::Maven,
            property_name: "p".to_string(),
            current_version: "1.0".to_string(),
            latest_version: Some("1.0".to_string()),
            outdated: false,
            skipped: false,
            error: None,
            artifact: None,
            kind: CheckerKind::Dependency,
        };
        assert_eq!(status_label(&r), "up-to-date");
    }

    #[test]
    fn json_output_structure() {
        let results = vec![CheckResult {
            ecosystem: Ecosystem::Maven,
            property_name: "version.junit".to_string(),
            current_version: "5.10.0".to_string(),
            latest_version: Some("5.12.0".to_string()),
            outdated: true,
            skipped: false,
            error: None,
            artifact: Some("org.junit.jupiter:junit-jupiter".to_string()),
            kind: CheckerKind::Dependency,
        }];

        let json_results: Vec<JsonResult> = results
            .iter()
            .map(|r| JsonResult {
                ecosystem: r.ecosystem.to_string().to_lowercase(),
                property: r.property_name.clone(),
                current: r.current_version.clone(),
                latest: r.latest_version.clone(),
                status: status_label(r).to_string(),
                kind: r.kind.to_string().to_lowercase(),
                error: r.error.clone(),
                artifact: r.artifact.clone(),
            })
            .collect();

        let json_str = serde_json::to_string(&json_results).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed[0]["status"], "outdated");
        assert_eq!(parsed[0]["kind"], "dependency");
        assert_eq!(parsed[0]["artifact"], "org.junit.jupiter:junit-jupiter");
    }

    #[test]
    fn json_output_includes_ecosystem() {
        let results = vec![CheckResult {
            ecosystem: Ecosystem::Maven,
            property_name: "version.junit".to_string(),
            current_version: "5.10.0".to_string(),
            latest_version: Some("5.12.0".to_string()),
            outdated: true,
            skipped: false,
            error: None,
            artifact: Some("org.junit.jupiter:junit-jupiter".to_string()),
            kind: CheckerKind::Dependency,
        }];

        let json_results: Vec<JsonResult> = results
            .iter()
            .map(|r| JsonResult {
                ecosystem: r.ecosystem.to_string().to_lowercase(),
                property: r.property_name.clone(),
                current: r.current_version.clone(),
                latest: r.latest_version.clone(),
                status: "outdated".to_string(),
                kind: r.kind.to_string().to_lowercase(),
                error: r.error.clone(),
                artifact: r.artifact.clone(),
            })
            .collect();

        let json_str = serde_json::to_string(&json_results).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed[0]["ecosystem"], "maven");
    }
}
