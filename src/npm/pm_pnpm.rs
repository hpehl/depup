//! pnpm package manager resolver.
//!
//! Uses `pnpm list --json` and `pnpm outdated --format json`.
//! pnpm's JSON output natively separates `dependencies` and `devDependencies`.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use serde::Deserialize;

use super::{InstalledPackage, OutdatedEntry, PackageManagerResolver};

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
    async fn list_packages(&self, dir: &Path) -> Result<Vec<InstalledPackage>> {
        let Some(entries) =
            super::run_pm_json::<Vec<ListOutput>>("pnpm", &["list", "--json", "--depth", "0"], dir)
                .await?
        else {
            return Ok(Vec::new());
        };

        let mut packages = Vec::new();
        for entry in entries {
            for (name, info) in entry.dependencies {
                packages.push(InstalledPackage {
                    name,
                    version: info.version,
                    is_dev: false,
                });
            }
            for (name, info) in entry.dev_dependencies {
                packages.push(InstalledPackage {
                    name,
                    version: info.version,
                    is_dev: true,
                });
            }
        }
        Ok(packages)
    }

    async fn outdated_packages(&self, dir: &Path) -> Result<HashMap<String, OutdatedEntry>> {
        super::outdated_json("pnpm", &["outdated", "--format", "json"], dir).await
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

    #[test]
    fn parse_list_rejects_malformed_json() {
        let result = serde_json::from_str::<Vec<ListOutput>>("not valid json");
        assert!(result.is_err());
    }

    #[test]
    fn parse_outdated_rejects_malformed_json() {
        let result = serde_json::from_str::<HashMap<String, OutdatedEntry>>("{{broken");
        assert!(result.is_err());
    }
}
