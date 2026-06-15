use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result};
use serde::Deserialize;
use tokio::process::Command;

use super::{OutdatedEntry, PackageManagerChecker, read_dev_dependency_names};

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

impl PackageManagerChecker for Yarn {
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
}
