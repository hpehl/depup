//! Resolves npm dependency versions against their package manager registries.
//!
//! Dispatches to the appropriate package manager implementation, runs
//! `list` and `outdated` commands concurrently via `tokio::try_join!`,
//! and merges the results into `VersionResult` values.

use std::path::Path;

use anyhow::Result;

use super::discovery::NpmProject;
use super::pm_version_check;
use super::{PackageManager, PackageManagerResolver, pm_bun, pm_npm, pm_pnpm, pm_yarn};
use crate::model::{CheckResult, CommandResult, Dependency, DependencyKind, Ecosystem};
use crate::version;

/// Runs `list_packages` and `outdated_packages` concurrently for any resolver.
async fn run_queries(
    resolver: &impl PackageManagerResolver,
    dir: &Path,
) -> Result<(
    Vec<(String, String, bool)>,
    std::collections::HashMap<String, super::OutdatedEntry>,
)> {
    tokio::try_join!(resolver.list_packages(dir), resolver.outdated_packages(dir))
}

/// Resolves versions for a single npm project.
/// Dispatches to the detected package manager and merges installed + outdated data.
pub async fn resolve_project(project: &NpmProject, root: &Path) -> Result<Vec<CheckResult>> {
    let source = project
        .path
        .strip_prefix(root)
        .unwrap_or(&project.path)
        .join("package.json")
        .display()
        .to_string();

    let (installed, outdated) = match project.package_manager {
        PackageManager::Npm => run_queries(&pm_npm::Npm, &project.path).await?,
        PackageManager::Pnpm => run_queries(&pm_pnpm::Pnpm, &project.path).await?,
        PackageManager::Yarn => run_queries(&pm_yarn::Yarn, &project.path).await?,
        PackageManager::Bun => run_queries(&pm_bun::Bun, &project.path).await?,
    };

    let mut results: Vec<CheckResult> = installed
        .into_iter()
        .map(|(name, current, is_dev)| {
            let kind = if is_dev {
                DependencyKind::NpmDevDep
            } else {
                DependencyKind::NpmDep
            };
            let id = Dependency::new(Ecosystem::Npm, kind, name.clone(), None, source.clone());
            if let Some(entry) = outdated.get(&id.artifact) {
                let is_outdated = version::is_newer(&entry.current, &entry.latest);
                CheckResult::checked(id, entry.current.clone(), entry.latest.clone(), is_outdated)
            } else {
                CheckResult::checked(id, current.clone(), current, false)
            }
        })
        .collect();

    if let Some(pm_result) = pm_version_check::check_pm_version(project, &source).await {
        results.push(pm_result);
    }

    results.sort_by(|a, b| a.artifact().cmp(b.artifact()));
    Ok(results)
}
