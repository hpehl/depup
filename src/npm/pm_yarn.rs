//! Yarn classic (v1) package manager resolver.
//!
//! Parses NDJSON from `yarn list --json` and `yarn outdated --json`.
//! Yarn classic outputs one JSON object per line (not a single JSON array),
//! requiring line-by-line parsing. Dev deps are classified from `package.json`.

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result};
use serde::Deserialize;
use tokio::process::Command;

use super::{OutdatedEntry, PackageManagerResolver, read_dev_dependency_names};

/// Yarn classic (v1) resolver implementation.
pub struct Yarn;

#[derive(Debug, Deserialize)]
struct TreeLine {
    #[serde(rename = "type")]
    line_type: String,
    data: Option<TreeData>,
}

#[derive(Debug, Deserialize)]
struct TreeData {
    #[serde(default)]
    trees: Vec<TreeEntry>,
}

#[derive(Debug, Deserialize)]
struct TreeEntry {
    name: String,
}

#[derive(Debug, Deserialize)]
struct OutdatedLine {
    #[serde(rename = "type")]
    line_type: String,
    data: Option<OutdatedTableData>,
}

#[derive(Debug, Deserialize)]
struct OutdatedTableData {
    body: Vec<Vec<String>>,
}

impl PackageManagerResolver for Yarn {
    async fn list_packages(&self, dir: &Path) -> Result<Vec<(String, String, bool)>> {
        let output = Command::new("yarn")
            .args(["list", "--json", "--depth", "0"])
            .current_dir(dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .with_context(|| format!("Failed to run 'yarn list' in {}", dir.display()))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim().is_empty() {
            return Ok(Vec::new());
        }

        let dev_deps = read_dev_dependency_names(dir);
        let mut packages = Vec::new();

        for line in stdout.lines() {
            let Ok(parsed) = serde_json::from_str::<TreeLine>(line) else {
                continue;
            };
            if parsed.line_type != "tree" {
                continue;
            }
            let Some(data) = parsed.data else { continue };
            for entry in data.trees {
                if let Some((name, version)) = parse_tree_name(&entry.name) {
                    let is_dev = dev_deps.contains(&name);
                    packages.push((name, version, is_dev));
                }
            }
        }
        Ok(packages)
    }

    async fn outdated_packages(&self, dir: &Path) -> Result<HashMap<String, OutdatedEntry>> {
        let output = Command::new("yarn")
            .args(["outdated", "--json"])
            .current_dir(dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .with_context(|| format!("Failed to run 'yarn outdated' in {}", dir.display()))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim().is_empty() {
            return Ok(HashMap::new());
        }

        let mut result = HashMap::new();
        for line in stdout.lines() {
            let Ok(parsed) = serde_json::from_str::<OutdatedLine>(line) else {
                continue;
            };
            if parsed.line_type != "table" {
                continue;
            }
            let Some(data) = parsed.data else { continue };
            // body rows: [package, current, wanted, latest, ...]
            for row in data.body {
                if row.len() >= 4 {
                    result.insert(
                        row[0].clone(),
                        OutdatedEntry {
                            current: row[1].clone(),
                            latest: row[3].clone(),
                        },
                    );
                }
            }
        }
        Ok(result)
    }

    async fn update_packages(&self, dir: &Path) -> Result<String> {
        let output = Command::new("yarn")
            .args(["upgrade"])
            .current_dir(dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .with_context(|| format!("Failed to run 'yarn upgrade' in {}", dir.display()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "yarn upgrade failed in {}: {}",
                dir.display(),
                stderr.trim()
            );
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

/// Parse "package@version" from yarn list tree entries
fn parse_tree_name(name: &str) -> Option<(String, String)> {
    let at_pos = name.rfind('@').filter(|&p| p > 0)?;
    let pkg = &name[..at_pos];
    let ver = &name[at_pos + 1..];
    if ver.is_empty() {
        return None;
    }
    Some((pkg.to_string(), ver.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tree_name_regular() {
        let (pkg, ver) = parse_tree_name("react@18.2.0").unwrap();
        assert_eq!(pkg, "react");
        assert_eq!(ver, "18.2.0");
    }

    #[test]
    fn parse_tree_name_scoped() {
        let (pkg, ver) = parse_tree_name("@types/node@20.0.0").unwrap();
        assert_eq!(pkg, "@types/node");
        assert_eq!(ver, "20.0.0");
    }

    #[test]
    fn parse_tree_name_no_version() {
        assert!(parse_tree_name("react").is_none());
    }

    #[test]
    fn parse_tree_name_trailing_at() {
        assert!(parse_tree_name("react@").is_none());
    }

    #[test]
    fn parse_tree_line_json() {
        let json = r#"{"type":"tree","data":{"trees":[{"name":"react@18.2.0"},{"name":"express@4.18.2"}]}}"#;
        let parsed: TreeLine = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.line_type, "tree");
        let data = parsed.data.unwrap();
        assert_eq!(data.trees.len(), 2);
        assert_eq!(data.trees[0].name, "react@18.2.0");
        assert_eq!(data.trees[1].name, "express@4.18.2");
    }

    #[test]
    fn parse_tree_line_non_tree_type() {
        let json = r#"{"type":"info","data":null}"#;
        let parsed: TreeLine = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.line_type, "info");
        assert!(parsed.data.is_none());
    }

    #[test]
    fn parse_outdated_line_json() {
        let json = r#"{"type":"table","data":{"body":[["react","18.0.0","18.2.0","19.0.0",""],["express","4.17.0","4.18.2","5.0.0",""]]}}"#;
        let parsed: OutdatedLine = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.line_type, "table");
        let data = parsed.data.unwrap();
        assert_eq!(data.body.len(), 2);
        assert_eq!(data.body[0][0], "react");
        assert_eq!(data.body[0][1], "18.0.0"); // current
        assert_eq!(data.body[0][3], "19.0.0"); // latest
        assert_eq!(data.body[1][0], "express");
    }

    #[test]
    fn parse_outdated_line_non_table_type() {
        let json = r#"{"type":"info","data":null}"#;
        let parsed: OutdatedLine = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.line_type, "info");
        assert!(parsed.data.is_none());
    }

    #[test]
    fn parse_tree_line_empty_trees() {
        let json = r#"{"type":"tree","data":{"trees":[]}}"#;
        let parsed: TreeLine = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.line_type, "tree");
        let data = parsed.data.unwrap();
        assert!(data.trees.is_empty());
    }
}
