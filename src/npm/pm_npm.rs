use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result};
use serde::Deserialize;
use tokio::process::Command;

use super::{OutdatedEntry, PackageManagerChecker, read_dev_dependency_names};

pub struct Npm;

#[derive(Debug, Deserialize)]
struct ListOutput {
    #[serde(default)]
    dependencies: HashMap<String, ListEntry>,
}

#[derive(Debug, Deserialize)]
struct ListEntry {
    #[serde(default)]
    version: String,
}

#[derive(Debug, Deserialize)]
struct OutdatedOutput {
    #[serde(default)]
    current: String,
    #[serde(default)]
    latest: String,
}

impl PackageManagerChecker for Npm {
    async fn list_packages(&self, dir: &Path) -> Result<Vec<(String, String, bool)>> {
        let output = Command::new("npm")
            .args(["list", "--json", "--depth", "0"])
            .current_dir(dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .with_context(|| format!("Failed to run 'npm list' in {}", dir.display()))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim().is_empty() {
            return Ok(Vec::new());
        }

        let list: ListOutput = serde_json::from_str(&stdout)
            .with_context(|| format!("Failed to parse npm list JSON in {}", dir.display()))?;

        let dev_deps = read_dev_dependency_names(dir);

        let packages = list
            .dependencies
            .into_iter()
            .map(|(name, entry)| {
                let is_dev = dev_deps.contains(&name);
                (name, entry.version, is_dev)
            })
            .collect();
        Ok(packages)
    }

    async fn outdated_packages(&self, dir: &Path) -> Result<HashMap<String, OutdatedEntry>> {
        let output = Command::new("npm")
            .args(["outdated", "--json"])
            .current_dir(dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .with_context(|| format!("Failed to run 'npm outdated' in {}", dir.display()))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim().is_empty() {
            return Ok(HashMap::new());
        }

        let packages: HashMap<String, OutdatedOutput> = serde_json::from_str(&stdout)
            .with_context(|| format!("Failed to parse npm outdated JSON in {}", dir.display()))?;

        Ok(packages
            .into_iter()
            .map(|(name, entry)| {
                (
                    name,
                    OutdatedEntry {
                        current: entry.current,
                        latest: entry.latest,
                    },
                )
            })
            .collect())
    }
}
