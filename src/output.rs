use comfy_table::{Cell, ContentArrangement, Table};
use console::style;

use crate::json::JsonResult;
use crate::registry::CheckResult;

pub fn print_table(results: &[CheckResult], verbose: bool) {
    if results.is_empty() {
        println!("{}", style("No version properties found.").yellow());
        return;
    }

    let mut table = Table::new();
    table.set_content_arrangement(ContentArrangement::Dynamic);

    let mut headers = vec!["Property", "Current", "Latest", "Status"];
    if verbose {
        headers.push("Artifact");
    }
    table.set_header(headers);

    for r in results {
        let (status_text, latest_text) = format_status(r);
        let mut row = vec![
            Cell::new(&r.property_name),
            Cell::new(&r.current_version),
            Cell::new(&latest_text),
            Cell::new(&status_text),
        ];
        if verbose {
            let artifact = r.artifact.as_deref().unwrap_or("");
            row.push(Cell::new(artifact));
        }
        table.add_row(row);
    }

    println!("{table}");
    print_summary(results);
}

pub fn print_json(results: &[CheckResult]) {
    let json_results: Vec<JsonResult> = results
        .iter()
        .map(|r| {
            let status = status_label(r);
            JsonResult {
                property: r.property_name.clone(),
                current: r.current_version.clone(),
                latest: r.latest_version.clone(),
                status: status.to_string(),
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

fn status_label(result: &CheckResult) -> &'static str {
    if result.error.is_some() {
        "error"
    } else if result.outdated {
        "outdated"
    } else {
        "up-to-date"
    }
}

fn format_status(result: &CheckResult) -> (String, String) {
    if let Some(err) = &result.error {
        let status = style("ERROR").red().to_string();
        let latest = style(err).red().to_string();
        (status, latest)
    } else if result.outdated {
        let status = style("OUTDATED").yellow().to_string();
        let latest = style(
            result
                .latest_version
                .as_deref()
                .unwrap_or("?"),
        )
        .yellow()
        .to_string();
        (status, latest)
    } else {
        let status = style("OK").green().to_string();
        let latest = style(
            result
                .latest_version
                .as_deref()
                .unwrap_or(&result.current_version),
        )
        .green()
        .to_string();
        (status, latest)
    }
}

fn print_summary(results: &[CheckResult]) {
    let total = results.len();
    let outdated = results.iter().filter(|r| r.outdated).count();
    let errors = results.iter().filter(|r| r.error.is_some()).count();
    let current = total - outdated - errors;

    println!();
    print!("{} properties checked: ", total);
    print!("{}", style(format!("{current} current")).green());
    if outdated > 0 {
        print!(", {}", style(format!("{outdated} outdated")).yellow());
    }
    if errors > 0 {
        print!(", {}", style(format!("{errors} errors")).red());
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_label_error() {
        let r = CheckResult {
            property_name: "p".to_string(),
            current_version: "1.0".to_string(),
            latest_version: None,
            outdated: false,
            error: Some("fail".to_string()),
            artifact: None,
        };
        assert_eq!(status_label(&r), "error");
    }

    #[test]
    fn status_label_outdated() {
        let r = CheckResult {
            property_name: "p".to_string(),
            current_version: "1.0".to_string(),
            latest_version: Some("2.0".to_string()),
            outdated: true,
            error: None,
            artifact: None,
        };
        assert_eq!(status_label(&r), "outdated");
    }

    #[test]
    fn status_label_up_to_date() {
        let r = CheckResult {
            property_name: "p".to_string(),
            current_version: "1.0".to_string(),
            latest_version: Some("1.0".to_string()),
            outdated: false,
            error: None,
            artifact: None,
        };
        assert_eq!(status_label(&r), "up-to-date");
    }

    #[test]
    fn json_output_structure() {
        let results = vec![CheckResult {
            property_name: "version.junit".to_string(),
            current_version: "5.10.0".to_string(),
            latest_version: Some("5.12.0".to_string()),
            outdated: true,
            error: None,
            artifact: Some("org.junit.jupiter:junit-jupiter".to_string()),
        }];

        let json_results: Vec<JsonResult> = results
            .iter()
            .map(|r| JsonResult {
                property: r.property_name.clone(),
                current: r.current_version.clone(),
                latest: r.latest_version.clone(),
                status: status_label(r).to_string(),
                error: r.error.clone(),
                artifact: r.artifact.clone(),
            })
            .collect();

        let json_str = serde_json::to_string(&json_results).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed[0]["status"], "outdated");
        assert_eq!(parsed[0]["artifact"], "org.junit.jupiter:junit-jupiter");
    }
}
