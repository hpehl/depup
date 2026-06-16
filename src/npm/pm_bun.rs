//! Bun package manager resolver.
//!
//! Lists packages by reading `package.json` + `node_modules/*/package.json`
//! (bun doesn't have a `list --json` command). Uses `bun outdated --format json`
//! for update information.

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result};
use serde::Deserialize;
use tokio::process::Command;

use super::{OutdatedEntry, PackageManagerResolver};

/// Bun resolver implementation.
pub struct Bun;

#[derive(Debug, Deserialize)]
struct OutdatedOutput {
    #[serde(default)]
    current: String,
    #[serde(default)]
    latest: String,
}

impl PackageManagerResolver for Bun {
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

    async fn update_packages(&self, dir: &Path) -> Result<String> {
        let output = Command::new("bun")
            .args(["update"])
            .current_dir(dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .with_context(|| format!("Failed to run 'bun update' in {}", dir.display()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("bun update failed in {}: {}", dir.display(), stderr.trim());
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

/// Reads the installed version of a package from its `node_modules/*/package.json`.
fn get_installed_version(dir: &Path, package: &str) -> Option<String> {
    let pkg_json = dir.join("node_modules").join(package).join("package.json");
    let content = std::fs::read_to_string(pkg_json).ok()?;
    let pkg: serde_json::Value = serde_json::from_str(&content).ok()?;
    pkg.get("version")
        .and_then(|v| v.as_str())
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn get_installed_version_found() {
        let tmp = TempDir::new().unwrap();
        let pkg_dir = tmp.path().join("node_modules").join("react");
        fs::create_dir_all(&pkg_dir).unwrap();
        fs::write(
            pkg_dir.join("package.json"),
            r#"{"name": "react", "version": "1.0.0"}"#,
        )
        .unwrap();

        let version = get_installed_version(tmp.path(), "react");
        assert_eq!(version, Some("1.0.0".to_string()));
    }

    #[test]
    fn get_installed_version_scoped_package() {
        let tmp = TempDir::new().unwrap();
        let pkg_dir = tmp.path().join("node_modules").join("@types/node");
        fs::create_dir_all(&pkg_dir).unwrap();
        fs::write(
            pkg_dir.join("package.json"),
            r#"{"name": "@types/node", "version": "20.0.0"}"#,
        )
        .unwrap();

        let version = get_installed_version(tmp.path(), "@types/node");
        assert_eq!(version, Some("20.0.0".to_string()));
    }

    #[test]
    fn get_installed_version_not_found() {
        let tmp = TempDir::new().unwrap();
        let version = get_installed_version(tmp.path(), "nonexistent-package");
        assert_eq!(version, None);
    }

    #[test]
    fn get_installed_version_malformed_json() {
        let tmp = TempDir::new().unwrap();
        let pkg_dir = tmp.path().join("node_modules").join("broken");
        fs::create_dir_all(&pkg_dir).unwrap();
        fs::write(pkg_dir.join("package.json"), "not json").unwrap();

        let version = get_installed_version(tmp.path(), "broken");
        assert_eq!(version, None);
    }

    #[test]
    fn get_installed_version_missing_version_field() {
        let tmp = TempDir::new().unwrap();
        let pkg_dir = tmp.path().join("node_modules").join("no-ver");
        fs::create_dir_all(&pkg_dir).unwrap();
        fs::write(pkg_dir.join("package.json"), r#"{"name": "no-ver"}"#).unwrap();

        let version = get_installed_version(tmp.path(), "no-ver");
        assert_eq!(version, None);
    }

    #[test]
    fn parse_outdated_output() {
        let json = r#"{"current":"4.18.2","latest":"5.0.0"}"#;
        let entry: OutdatedOutput = serde_json::from_str(json).unwrap();
        assert_eq!(entry.current, "4.18.2");
        assert_eq!(entry.latest, "5.0.0");
    }

    #[test]
    fn parse_outdated_output_as_map() {
        let json = r#"{"lodash":{"current":"4.17.21","latest":"5.0.0"}}"#;
        let packages: HashMap<String, OutdatedOutput> = serde_json::from_str(json).unwrap();
        assert_eq!(packages.len(), 1);
        assert_eq!(packages["lodash"].current, "4.17.21");
        assert_eq!(packages["lodash"].latest, "5.0.0");
    }
}
