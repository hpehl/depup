//! npm package manager resolver.
//!
//! Uses `npm list --json` and `npm outdated --json` for package data.
//! Dev dependencies are classified by reading `devDependencies` from `package.json`
//! since `npm list` doesn't distinguish them.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use serde::Deserialize;

use super::{InstalledPackage, OutdatedEntry, PackageManagerResolver, read_dev_dependency_names};

/// npm resolver implementation.
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

impl PackageManagerResolver for Npm {
    async fn list_packages(&self, dir: &Path) -> Result<Vec<InstalledPackage>> {
        let Some(list) =
            super::run_pm_json::<ListOutput>("npm", &["list", "--json", "--depth", "0"], dir)
                .await?
        else {
            return Ok(Vec::new());
        };

        let dev_deps = read_dev_dependency_names(dir);

        let packages = list
            .dependencies
            .into_iter()
            .map(|(name, entry)| {
                let is_dev = dev_deps.contains(&name);
                InstalledPackage {
                    name,
                    version: entry.version,
                    is_dev,
                }
            })
            .collect();
        Ok(packages)
    }

    async fn outdated_packages(&self, dir: &Path) -> Result<HashMap<String, OutdatedEntry>> {
        super::outdated_json("npm", &["outdated", "--json"], dir).await
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_list_output() {
        let json =
            r#"{"dependencies":{"react":{"version":"18.2.0"},"express":{"version":"4.18.2"}}}"#;
        let list: ListOutput = serde_json::from_str(json).unwrap();
        assert_eq!(list.dependencies.len(), 2);
        assert_eq!(list.dependencies["react"].version, "18.2.0");
        assert_eq!(list.dependencies["express"].version, "4.18.2");
    }

    #[test]
    fn parse_list_output_empty_deps() {
        let json = r#"{"dependencies":{}}"#;
        let list: ListOutput = serde_json::from_str(json).unwrap();
        assert!(list.dependencies.is_empty());
    }

    #[test]
    fn parse_list_output_missing_deps_field() {
        let json = r#"{}"#;
        let list: ListOutput = serde_json::from_str(json).unwrap();
        assert!(list.dependencies.is_empty());
    }

    #[test]
    fn parse_outdated_entry() {
        let json = r#"{"current":"4.18.2","latest":"5.0.0"}"#;
        let entry: OutdatedEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.current, "4.18.2");
        assert_eq!(entry.latest, "5.0.0");
    }

    #[test]
    fn parse_outdated_entry_as_map() {
        let json = r#"{"express":{"current":"4.18.2","latest":"5.0.0"},"react":{"current":"18.2.0","latest":"19.0.0"}}"#;
        let packages: HashMap<String, OutdatedEntry> = serde_json::from_str(json).unwrap();
        assert_eq!(packages.len(), 2);
        assert_eq!(packages["express"].current, "4.18.2");
        assert_eq!(packages["express"].latest, "5.0.0");
        assert_eq!(packages["react"].current, "18.2.0");
        assert_eq!(packages["react"].latest, "19.0.0");
    }

    #[test]
    fn parse_outdated_entry_defaults() {
        let json = r#"{}"#;
        let entry: OutdatedEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.current, "");
        assert_eq!(entry.latest, "");
    }

    #[test]
    fn parse_list_rejects_malformed_json() {
        let result = serde_json::from_str::<ListOutput>("not valid json {{{");
        assert!(result.is_err());
    }

    #[test]
    fn parse_outdated_rejects_malformed_json() {
        let result = serde_json::from_str::<HashMap<String, OutdatedEntry>>("not valid json");
        assert!(result.is_err());
    }
}
