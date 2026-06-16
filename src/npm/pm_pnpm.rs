//! pnpm package manager resolver.
//!
//! Uses `pnpm list --json` and `pnpm outdated --format json`.
//! pnpm's JSON output natively separates `dependencies` and `devDependencies`.

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result};
use serde::Deserialize;
use tokio::process::Command;

use super::{OutdatedEntry, PackageManagerResolver};

/// pnpm resolver implementation.
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

impl PackageManagerResolver for Pnpm {
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

    async fn update_packages(&self, dir: &Path) -> Result<String> {
        let output = Command::new("pnpm")
            .args(["update"])
            .current_dir(dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .with_context(|| format!("Failed to run 'pnpm update' in {}", dir.display()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("pnpm update failed in {}: {}", dir.display(), stderr.trim());
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_list_output_array() {
        let json = r#"[{"dependencies":{"react":{"version":"18.2.0"}},"devDependencies":{"vitest":{"version":"1.0.0"}}}]"#;
        let entries: Vec<ListOutput> = serde_json::from_str(json).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].dependencies.len(), 1);
        assert_eq!(entries[0].dependencies["react"].version, "18.2.0");
        assert_eq!(entries[0].dev_dependencies.len(), 1);
        assert_eq!(entries[0].dev_dependencies["vitest"].version, "1.0.0");
    }

    #[test]
    fn parse_list_output_empty_array() {
        let json = r#"[]"#;
        let entries: Vec<ListOutput> = serde_json::from_str(json).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn parse_list_output_no_dev_deps() {
        let json = r#"[{"dependencies":{"express":{"version":"4.18.2"}}}]"#;
        let entries: Vec<ListOutput> = serde_json::from_str(json).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].dependencies["express"].version, "4.18.2");
        assert!(entries[0].dev_dependencies.is_empty());
    }

    #[test]
    fn parse_list_output_multiple_entries() {
        let json = r#"[
            {"dependencies":{"react":{"version":"18.2.0"}},"devDependencies":{}},
            {"dependencies":{"express":{"version":"4.18.2"}},"devDependencies":{"jest":{"version":"29.0.0"}}}
        ]"#;
        let entries: Vec<ListOutput> = serde_json::from_str(json).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].dependencies["react"].version, "18.2.0");
        assert_eq!(entries[1].dependencies["express"].version, "4.18.2");
        assert_eq!(entries[1].dev_dependencies["jest"].version, "29.0.0");
    }
}
