pub mod discovery;
mod pm_bun;
mod pm_npm;
mod pm_pnpm;
mod pm_yarn;

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::Result;

use crate::registry::{CheckResult, CheckerKind, Ecosystem};
use crate::version;

use discovery::NpmProject;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    Npm,
    Pnpm,
    Yarn,
    Bun,
}

impl PackageManager {
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

#[derive(Debug, Clone)]
pub struct OutdatedEntry {
    pub current: String,
    pub latest: String,
}

pub trait PackageManagerChecker {
    async fn list_packages(&self, dir: &Path) -> Result<Vec<(String, String, bool)>>;
    async fn outdated_packages(&self, dir: &Path) -> Result<HashMap<String, OutdatedEntry>>;
}

pub fn read_dev_dependency_names(dir: &Path) -> HashSet<String> {
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

async fn run_checks(
    checker: &impl PackageManagerChecker,
    dir: &Path,
) -> Result<(Vec<(String, String, bool)>, HashMap<String, OutdatedEntry>)> {
    tokio::try_join!(checker.list_packages(dir), checker.outdated_packages(dir))
}

pub async fn check_project(project: &NpmProject, root: &Path) -> Result<Vec<CheckResult>> {
    let source = project
        .path
        .strip_prefix(root)
        .unwrap_or(&project.path)
        .join("package.json")
        .display()
        .to_string();

    let (installed, outdated) = match project.package_manager {
        PackageManager::Npm => run_checks(&pm_npm::Npm, &project.path).await?,
        PackageManager::Pnpm => run_checks(&pm_pnpm::Pnpm, &project.path).await?,
        PackageManager::Yarn => run_checks(&pm_yarn::Yarn, &project.path).await?,
        PackageManager::Bun => run_checks(&pm_bun::Bun, &project.path).await?,
    };

    let mut results: Vec<CheckResult> = installed
        .into_iter()
        .map(|(name, current, is_dev)| {
            let kind = if is_dev {
                CheckerKind::NpmDevDep
            } else {
                CheckerKind::NpmDep
            };
            if let Some(entry) = outdated.get(&name) {
                let is_outdated = version::is_newer(&entry.current, &entry.latest);
                CheckResult::checked(
                    Ecosystem::Npm,
                    kind,
                    name.clone(),
                    entry.current.clone(),
                    entry.latest.clone(),
                    is_outdated,
                    Some(name),
                    source.clone(),
                )
            } else {
                CheckResult::checked(
                    Ecosystem::Npm,
                    kind,
                    name.clone(),
                    current.clone(),
                    current,
                    false,
                    Some(name),
                    source.clone(),
                )
            }
        })
        .collect();

    results.sort_by(|a, b| a.property_name.cmp(&b.property_name));
    Ok(results)
}
