pub mod discovery;

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result};
use serde::Deserialize;
use tokio::process::Command;

use crate::registry::{CheckResult, CheckerKind, Ecosystem};
use crate::version;

#[derive(Debug, Deserialize)]
struct ListOutput {
    #[serde(default)]
    dependencies: HashMap<String, ListEntry>,
    #[serde(rename = "devDependencies", default)]
    dev_dependencies: HashMap<String, ListEntry>,
}

#[derive(Debug, Deserialize)]
struct ListEntry {
    version: String,
}

#[derive(Debug, Deserialize)]
struct OutdatedEntry {
    current: String,
    latest: String,
}

pub async fn check_project(project_dir: &Path, root: &Path) -> Result<Vec<CheckResult>> {
    let source = project_dir
        .strip_prefix(root)
        .unwrap_or(project_dir)
        .join("package.json")
        .display()
        .to_string();

    let (installed, outdated) =
        tokio::try_join!(list_packages(project_dir), outdated_packages(project_dir),)?;

    let mut results: Vec<CheckResult> = installed
        .into_iter()
        .map(|(name, current)| {
            if let Some(entry) = outdated.get(&name) {
                let is_outdated = version::is_newer(&entry.current, &entry.latest);
                CheckResult::checked(
                    Ecosystem::Pnpm,
                    CheckerKind::Pnpm,
                    name.clone(),
                    entry.current.clone(),
                    entry.latest.clone(),
                    is_outdated,
                    Some(name),
                )
            } else {
                CheckResult::checked(
                    Ecosystem::Pnpm,
                    CheckerKind::Pnpm,
                    name.clone(),
                    current.clone(),
                    current,
                    false,
                    Some(name),
                )
            }
            .with_source(source.clone())
        })
        .collect();

    results.sort_by(|a, b| a.property_name.cmp(&b.property_name));
    Ok(results)
}

async fn list_packages(project_dir: &Path) -> Result<Vec<(String, String)>> {
    let output = Command::new("pnpm")
        .args(["list", "--json", "--depth", "0"])
        .current_dir(project_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .with_context(|| format!("Failed to run 'pnpm list' in {}", project_dir.display()))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Ok(Vec::new());
    }

    let entries: Vec<ListOutput> = serde_json::from_str(&stdout).with_context(|| {
        format!(
            "Failed to parse pnpm list JSON in {}",
            project_dir.display()
        )
    })?;

    let mut packages = Vec::new();
    for entry in entries {
        for (name, info) in entry.dependencies {
            packages.push((name, info.version));
        }
        for (name, info) in entry.dev_dependencies {
            packages.push((name, info.version));
        }
    }
    Ok(packages)
}

async fn outdated_packages(
    project_dir: &Path,
) -> Result<HashMap<String, OutdatedEntry>> {
    let output = Command::new("pnpm")
        .args(["outdated", "--format", "json"])
        .current_dir(project_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .with_context(|| {
            format!(
                "Failed to run 'pnpm outdated' in {}",
                project_dir.display()
            )
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Ok(HashMap::new());
    }

    let packages: HashMap<String, OutdatedEntry> =
        serde_json::from_str(&stdout).with_context(|| {
            format!(
                "Failed to parse pnpm outdated JSON in {}",
                project_dir.display()
            )
        })?;

    Ok(packages)
}
