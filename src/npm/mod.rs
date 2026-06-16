//! npm ecosystem support.
//!
//! Discovers npm/pnpm/yarn/bun projects in a directory tree and checks for
//! outdated packages. Auto-detects the package manager by lock file or
//! `packageManager` field in `package.json`.
//!
//! - [`PackageManagerResolver`] trait — each PM implements `list_packages()` and `outdated_packages()`.
//! - [`resolver`] module — dispatches to the detected PM and resolves versions into [`crate::model::CheckResult`]s.
//! - [`discovery`] module — walks the directory tree finding npm projects.

pub mod discovery;
mod pm_bun;
mod pm_npm;
mod pm_pnpm;
pub mod pm_version_check;
mod pm_yarn;
pub mod resolver;
pub mod updater;

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::Result;

/// Supported npm ecosystem package managers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    Npm,
    Pnpm,
    Yarn,
    Bun,
}

impl PackageManager {
    /// Returns the CLI command name for this package manager.
    pub fn command(self) -> &'static str {
        match self {
            Self::Npm => "npm",
            Self::Pnpm => "pnpm",
            Self::Yarn => "yarn",
            Self::Bun => "bun",
        }
    }
}

impl std::fmt::Display for PackageManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.command())
    }
}

/// A package that has a newer version available.
#[derive(Debug, Clone)]
pub struct OutdatedEntry {
    pub current: String,
    pub latest: String,
}

/// Trait for package-manager-specific operations: listing installed packages
/// and querying for outdated packages. Each PM implements this with its own
/// CLI commands and JSON output format.
pub trait PackageManagerResolver {
    /// Lists installed packages as `(name, version, is_dev)` tuples.
    async fn list_packages(&self, dir: &Path) -> Result<Vec<(String, String, bool)>>;
    /// Queries for outdated packages, returning a map of package name to outdated info.
    async fn outdated_packages(&self, dir: &Path) -> Result<HashMap<String, OutdatedEntry>>;
    /// Runs the package manager's native update command, returning stdout.
    async fn update_packages(&self, dir: &Path) -> Result<String>;
}

/// Reads `devDependencies` keys from `package.json` to classify dev vs. prod deps.
/// Used by npm and yarn resolvers that don't distinguish dev deps in their `list` output.
pub(super) fn read_dev_dependency_names(dir: &Path) -> HashSet<String> {
    let Ok(content) = std::fs::read_to_string(dir.join("package.json")) else {
        return HashSet::new();
    };
    let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content) else {
        return HashSet::new();
    };
    pkg.get("devDependencies")
        .and_then(|v| v.as_object())
        .map(|obj| obj.keys().cloned().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn read_dev_dependency_names_with_dev_deps() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("package.json"),
            r#"{
                "dependencies": {"react": "^18.0.0"},
                "devDependencies": {"vitest": "^1.0.0", "eslint": "^8.0.0"}
            }"#,
        )
        .unwrap();

        let names = read_dev_dependency_names(tmp.path());
        assert_eq!(names.len(), 2);
        assert!(names.contains("vitest"));
        assert!(names.contains("eslint"));
    }

    #[test]
    fn read_dev_dependency_names_without_dev_deps() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("package.json"),
            r#"{"dependencies": {"react": "^18.0.0"}}"#,
        )
        .unwrap();

        let names = read_dev_dependency_names(tmp.path());
        assert!(names.is_empty());
    }

    #[test]
    fn read_dev_dependency_names_malformed_json() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("package.json"), "not valid json {{{").unwrap();

        let names = read_dev_dependency_names(tmp.path());
        assert!(names.is_empty());
    }

    #[test]
    fn read_dev_dependency_names_nonexistent_dir() {
        let names = read_dev_dependency_names(Path::new("/nonexistent/path/that/does/not/exist"));
        assert!(names.is_empty());
    }
}
