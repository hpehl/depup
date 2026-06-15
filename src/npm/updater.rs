//! Orchestrates npm ecosystem updates by delegating to each project's
//! package manager native update command.

use std::path::Path;

use super::discovery::NpmProject;
use super::{PackageManager, PackageManagerChecker, pm_bun, pm_npm, pm_pnpm, pm_yarn};
use crate::dependency::{UpdateResult, VersionResult};

/// Runs the native update command for a single npm project and maps the
/// outcome back to one `UpdateResult` per outdated dependency.
pub async fn update_project(
    project: &NpmProject,
    _root: &Path,
    outdated: &[VersionResult],
) -> Vec<UpdateResult> {
    let result = match project.package_manager {
        PackageManager::Npm => pm_npm::Npm.update_packages(&project.path).await,
        PackageManager::Pnpm => pm_pnpm::Pnpm.update_packages(&project.path).await,
        PackageManager::Yarn => pm_yarn::Yarn.update_packages(&project.path).await,
        PackageManager::Bun => pm_bun::Bun.update_packages(&project.path).await,
    };

    match result {
        Ok(_) => outdated
            .iter()
            .map(|r| UpdateResult::updated(r, r.latest_version().unwrap_or("").to_string()))
            .collect(),
        Err(e) => outdated
            .iter()
            .map(|r| UpdateResult::error(r, e.to_string()))
            .collect(),
    }
}
