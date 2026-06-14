pub mod discovery;

use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result};
use serde::Deserialize;
use tokio::process::Command;

use crate::registry::{CheckResult, CheckerKind, Ecosystem};
use crate::version;

#[derive(Debug, Deserialize)]
struct OutdatedEntry {
    current: String,
    latest: String,
}

pub async fn check_project(project_dir: &Path) -> Result<Vec<CheckResult>> {
    let output = Command::new("pnpm")
        .arg("outdated")
        .arg("--format")
        .arg("json")
        .current_dir(project_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .with_context(|| format!("Failed to run 'pnpm outdated' in {}", project_dir.display()))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // pnpm outdated exits with 1 when packages are outdated — that's not an error
    if stdout.trim().is_empty() {
        return Ok(Vec::new());
    }

    let packages: std::collections::HashMap<String, OutdatedEntry> = serde_json::from_str(&stdout)
        .with_context(|| {
            format!(
                "Failed to parse pnpm outdated JSON in {}",
                project_dir.display()
            )
        })?;

    let mut results: Vec<CheckResult> = packages
        .into_iter()
        .map(|(name, entry)| {
            let is_outdated = version::is_newer(&entry.current, &entry.latest);
            CheckResult::checked(
                Ecosystem::Pnpm,
                CheckerKind::Pnpm,
                name.clone(),
                entry.current,
                entry.latest,
                is_outdated,
                Some(name),
            )
        })
        .collect();

    results.sort_by(|a, b| a.property_name.cmp(&b.property_name));
    Ok(results)
}
