//! pnpm package manager checker.
//!
//! Uses `pnpm list --json` and `pnpm outdated --format json`.
//! pnpm's JSON output natively separates `dependencies` and `devDependencies`.

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result};
use serde::Deserialize;
use tokio::process::Command;

use super::{OutdatedEntry, PackageManagerChecker};

/// pnpm checker implementation.
pub struct Pnpm;

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

impl PackageManagerChecker for Pnpm {
    async fn list_packages(&self, dir: &Path) -> Result<Vec<(String, String, bool)>> {
        let output = Command::new("pnpm")
            .args(["list", "--json", "--depth", "0"])
            .current_dir(dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .with_context(|| format!("Failed to run 'pnpm list' in {}", dir.display()))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim().is_empty() {
            return Ok(Vec::new());
        }

        let entries: Vec<ListOutput> = serde_json::from_str(&stdout)
            .with_context(|| format!("Failed to parse pnpm list JSON in {}", dir.display()))?;

        let mut packages = Vec::new();
        for entry in entries {
            for (name, info) in entry.dependencies {
                packages.push((name, info.version, false));
            }
            for (name, info) in entry.dev_dependencies {
                packages.push((name, info.version, true));
            }
        }
        Ok(packages)
    }

    async fn outdated_packages(&self, dir: &Path) -> Result<HashMap<String, OutdatedEntry>> {
        let output = Command::new("pnpm")
            .args(["outdated", "--format", "json"])
            .current_dir(dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .with_context(|| format!("Failed to run 'pnpm outdated' in {}", dir.display()))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim().is_empty() {
            return Ok(HashMap::new());
        }

        #[derive(Debug, Deserialize)]
        struct PnpmOutdatedEntry {
            current: String,
            latest: String,
        }

        let packages: HashMap<String, PnpmOutdatedEntry> = serde_json::from_str(&stdout)
            .with_context(|| format!("Failed to parse pnpm outdated JSON in {}", dir.display()))?;

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
