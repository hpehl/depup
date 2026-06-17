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
pub(crate) mod pm_version_check;
mod pm_yarn;
pub mod resolver;
pub mod updater;

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use tokio::process::Command;

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

    /// Runs `list_packages` and `outdated_packages` concurrently for this PM.
    pub(crate) async fn run_queries(
        self,
        dir: &Path,
    ) -> Result<(Vec<InstalledPackage>, HashMap<String, OutdatedEntry>)> {
        match self {
            Self::Npm => tokio::try_join!(
                pm_npm::Npm.list_packages(dir),
                pm_npm::Npm.outdated_packages(dir)
            ),
            Self::Pnpm => tokio::try_join!(
                pm_pnpm::Pnpm.list_packages(dir),
                pm_pnpm::Pnpm.outdated_packages(dir)
            ),
            Self::Yarn => tokio::try_join!(
                pm_yarn::Yarn.list_packages(dir),
                pm_yarn::Yarn.outdated_packages(dir)
            ),
            Self::Bun => tokio::try_join!(
                pm_bun::Bun.list_packages(dir),
                pm_bun::Bun.outdated_packages(dir)
            ),
        }
    }

    /// Runs the PM's native update command.
    pub(crate) async fn update(self, dir: &Path) -> Result<String> {
        match self {
            Self::Npm => pm_npm::Npm.update_packages(dir).await,
            Self::Pnpm => pm_pnpm::Pnpm.update_packages(dir).await,
            Self::Yarn => pm_yarn::Yarn.update_packages(dir).await,
            Self::Bun => pm_bun::Bun.update_packages(dir).await,
        }
    }
}

impl std::fmt::Display for PackageManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.command())
    }
}

/// A single installed package from a package manager's list output.
#[derive(Debug, Clone)]
pub struct InstalledPackage {
    pub name: String,
    pub version: String,
    pub is_dev: bool,
}

/// A package that has a newer version available.
#[derive(Debug, Clone, Deserialize)]
pub struct OutdatedEntry {
    #[serde(default)]
    pub current: String,
    #[serde(default)]
    pub latest: String,
}

/// Trait for package-manager-specific operations: listing installed packages
/// and querying for outdated packages. Each PM implements this with its own
/// CLI commands and JSON output format.
pub trait PackageManagerResolver {
    /// Lists installed packages with name, version, and dev classification.
    async fn list_packages(&self, dir: &Path) -> Result<Vec<InstalledPackage>>;
    /// Queries for outdated packages, returning a map of package name to outdated info.
    async fn outdated_packages(&self, dir: &Path) -> Result<HashMap<String, OutdatedEntry>>;
    /// Runs the package manager's native update command, returning stdout.
    async fn update_packages(&self, dir: &Path) -> Result<String>;
}

/// Runs a PM's `outdated` command and parses the JSON result into an `OutdatedEntry` map.
pub(crate) async fn outdated_json(
    pm_name: &str,
    args: &[&str],
    dir: &Path,
) -> Result<HashMap<String, OutdatedEntry>> {
    Ok(
        run_pm_json::<HashMap<String, OutdatedEntry>>(pm_name, args, dir)
            .await?
            .unwrap_or_default(),
    )
}

/// Runs a package manager command and returns stdout, or errors on non-zero exit.
pub(crate) async fn run_pm_command(pm_name: &str, args: &[&str], dir: &Path) -> Result<String> {
    let safe_dir = dir
        .canonicalize()
        .with_context(|| format!("Invalid directory path: {}", dir.display()))?;
    let output = Command::new(pm_name)
        .args(args)
        .current_dir(&safe_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .with_context(|| {
            format!(
                "Failed to run '{} {}' in {}",
                pm_name,
                args.join(" "),
                dir.display()
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "{} {} failed in {}: {}",
            pm_name,
            args.join(" "),
            dir.display(),
            stderr.trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Runs a package manager command, parses the JSON stdout, and returns `None` if stdout is empty.
///
/// Does not fail on non-zero exit codes because some commands (e.g. `npm outdated`)
/// return exit code 1 when outdated packages exist while still producing valid JSON.
pub(crate) async fn run_pm_json<T: DeserializeOwned>(
    pm_name: &str,
    args: &[&str],
    dir: &Path,
) -> Result<Option<T>> {
    let safe_dir = dir
        .canonicalize()
        .with_context(|| format!("Invalid directory path: {}", dir.display()))?;
    let output = Command::new(pm_name)
        .args(args)
        .current_dir(&safe_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .with_context(|| {
            format!(
                "Failed to run '{} {}' in {}",
                pm_name,
                args.join(" "),
                dir.display()
            )
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Ok(None);
    }

    let parsed = serde_json::from_str(&stdout).with_context(|| {
        format!(
            "Failed to parse '{} {}' JSON in {}",
            pm_name,
            args.join(" "),
            dir.display()
        )
    })?;
    Ok(Some(parsed))
}

/// Reads `devDependencies` keys from `package.json` to classify dev vs. prod deps.
/// Used by npm and yarn resolvers that don't distinguish dev deps in their `list` output.
pub(super) fn read_dev_dependency_names(dir: &Path) -> HashSet<String> {
    let path = dir.join("package.json");
    let Ok(content) = std::fs::read_to_string(&path) else {
        return HashSet::new();
    };
    let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content) else {
        eprintln!(
            "Warning: failed to parse {}",
            path.display()
        );
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
