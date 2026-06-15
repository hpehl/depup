use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result};
use serde::Deserialize;
use tokio::process::Command;

use super::{OutdatedEntry, PackageManagerChecker};

pub struct Bun;

#[derive(Debug, Deserialize)]
struct OutdatedOutput {
    #[serde(default)]
    current: String,
    #[serde(default)]
    latest: String,
}

impl PackageManagerChecker for Bun {
    async fn list_packages(&self, dir: &Path) -> Result<Vec<(String, String, bool)>> {
        let pkg_content = std::fs::read_to_string(dir.join("package.json"))
            .with_context(|| format!("Failed to read package.json in {}", dir.display()))?;
        let pkg: serde_json::Value = serde_json::from_str(&pkg_content)
            .with_context(|| format!("Failed to parse package.json in {}", dir.display()))?;

        let mut packages = Vec::new();

        if let Some(deps) = pkg.get("dependencies").and_then(|v| v.as_object()) {
            for (name, _) in deps {
                let version = get_installed_version(dir, name).unwrap_or_default();
                packages.push((name.clone(), version, false));
            }
        }

        if let Some(deps) = pkg.get("devDependencies").and_then(|v| v.as_object()) {
            for (name, _) in deps {
                let version = get_installed_version(dir, name).unwrap_or_default();
                packages.push((name.clone(), version, true));
            }
        }

        Ok(packages)
    }

    async fn outdated_packages(&self, dir: &Path) -> Result<HashMap<String, OutdatedEntry>> {
        let output = Command::new("bun")
            .args(["outdated", "--format", "json"])
            .current_dir(dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .with_context(|| format!("Failed to run 'bun outdated' in {}", dir.display()))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim().is_empty() {
            return Ok(HashMap::new());
        }

        let packages: HashMap<String, OutdatedOutput> = serde_json::from_str(&stdout)
            .with_context(|| format!("Failed to parse bun outdated JSON in {}", dir.display()))?;

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

fn get_installed_version(dir: &Path, package: &str) -> Option<String> {
    let pkg_json = dir.join("node_modules").join(package).join("package.json");
    let content = std::fs::read_to_string(pkg_json).ok()?;
    let pkg: serde_json::Value = serde_json::from_str(&content).ok()?;
    pkg.get("version")
        .and_then(|v| v.as_str())
        .map(String::from)
}
