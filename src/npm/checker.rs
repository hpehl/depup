//! npm ecosystem checker.
//!
//! Dispatches to the appropriate package manager implementation, runs
//! `list` and `outdated` commands concurrently via `tokio::try_join!`,
//! and merges the results into `CheckResult` values.

use std::path::Path;

use anyhow::Result;

use super::discovery::NpmProject;
use super::{PackageManager, PackageManagerChecker, pm_bun, pm_npm, pm_pnpm, pm_yarn};
use crate::registry::{CheckId, CheckResult, CheckerKind, Ecosystem};
use crate::version;

/// Runs `list_packages` and `outdated_packages` concurrently for any checker.
async fn run_checks(
    checker: &impl PackageManagerChecker,
    dir: &Path,
) -> Result<(
    Vec<(String, String, bool)>,
    std::collections::HashMap<String, super::OutdatedEntry>,
)> {
    tokio::try_join!(checker.list_packages(dir), checker.outdated_packages(dir))
}

/// Checks a single npm project for outdated dependencies.
/// Dispatches to the detected package manager and merges installed + outdated data.
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
            let id = CheckId::new(
                Ecosystem::Npm,
                kind,
                name.clone(),
                Some(name),
                source.clone(),
            );
            if let Some(entry) = outdated.get(&id.property_name) {
                let is_outdated = version::is_newer(&entry.current, &entry.latest);
                CheckResult::checked(id, entry.current.clone(), entry.latest.clone(), is_outdated)
            } else {
                CheckResult::checked(id, current.clone(), current, false)
            }
        })
        .collect();

    results.sort_by(|a, b| a.property_name().cmp(b.property_name()));
    Ok(results)
}
