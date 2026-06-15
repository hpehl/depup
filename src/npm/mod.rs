//! npm ecosystem support.
//!
//! Discovers npm/pnpm/yarn/bun projects in a directory tree and checks for
//! outdated packages. Auto-detects the package manager by lock file or
//! `packageManager` field in `package.json`. Each package manager implements
//! the [`PackageManagerChecker`] trait.

pub mod checker;
pub mod discovery;
mod pm_bun;
mod pm_npm;
mod pm_pnpm;
mod pm_yarn;

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
pub trait PackageManagerChecker {
    /// Lists installed packages as `(name, version, is_dev)` tuples.
    async fn list_packages(&self, dir: &Path) -> Result<Vec<(String, String, bool)>>;
    /// Queries for outdated packages, returning a map of package name to outdated info.
    async fn outdated_packages(&self, dir: &Path) -> Result<HashMap<String, OutdatedEntry>>;
}

/// Reads `devDependencies` keys from `package.json` to classify dev vs. prod deps.
/// Used by npm and yarn checkers that don't distinguish dev deps in their `list` output.
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
