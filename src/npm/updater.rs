//! Orchestrates npm ecosystem updates by delegating to each project's
//! package manager native update command and rewriting the `packageManager`
//! field in `package.json` when the PM version itself is outdated.

use std::path::Path;

use super::discovery::NpmProject;
use super::pm_version_check;
use super::{PackageManager, PackageManagerResolver, pm_bun, pm_npm, pm_pnpm, pm_yarn};
use crate::model::{CommandResult, DependencyKind, UpdateResult, CheckResult};

/// Runs the native update command for a single npm project and maps the
/// outcome back to one `UpdateResult` per outdated dependency.
pub async fn update_project(
    project: &NpmProject,
    _root: &Path,
    outdated: &[CheckResult],
) -> Vec<UpdateResult> {
    let (tool_versions, package_deps): (Vec<_>, Vec<_>) = outdated
        .iter()
        .partition(|r| r.kind() == DependencyKind::Tool);

    let mut results = Vec::new();

    // Update regular package dependencies via the PM's native update command
    if !package_deps.is_empty() {
        let result = match project.package_manager {
            PackageManager::Npm => pm_npm::Npm.update_packages(&project.path).await,
            PackageManager::Pnpm => pm_pnpm::Pnpm.update_packages(&project.path).await,
            PackageManager::Yarn => pm_yarn::Yarn.update_packages(&project.path).await,
            PackageManager::Bun => pm_bun::Bun.update_packages(&project.path).await,
        };

        match result {
            Ok(_) => {
                results.extend(package_deps.iter().map(|r| {
                    UpdateResult::updated(r, r.latest_version().unwrap_or("").to_string())
                }))
            }
            Err(e) => results.extend(
                package_deps
                    .iter()
                    .map(|r| UpdateResult::error(r, e.to_string())),
            ),
        }
    }

    // Update the packageManager field in package.json
    for r in &tool_versions {
        let new_version = r.latest_version().unwrap_or("");
        if new_version.is_empty() {
            results.push(UpdateResult::error(r, "No latest version available".into()));
            continue;
        }
        match pm_version_check::update_pm_version(
            &project.path,
            project.package_manager.command(),
            new_version,
        ) {
            Ok(()) => results.push(UpdateResult::updated(r, new_version.to_string())),
            Err(e) => results.push(UpdateResult::error(r, e.to_string())),
        }
    }

    results
}
