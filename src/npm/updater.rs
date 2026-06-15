//! Orchestrates npm ecosystem updates by delegating to each project's
//! package manager native update command.

use std::path::Path;

use super::discovery::NpmProject;
use super::{PackageManager, PackageManagerChecker, pm_bun, pm_npm, pm_pnpm, pm_yarn};

/// Summary of an npm project update.
#[derive(Debug)]
pub struct NpmUpdateResult {
    pub project_name: String,
    pub package_manager: PackageManager,
    pub success: bool,
    pub message: String,
}

/// Runs the native update command for a single npm project.
pub async fn update_project(project: &NpmProject, root: &Path) -> NpmUpdateResult {
    let relative = project
        .path
        .strip_prefix(root)
        .unwrap_or(&project.path)
        .display()
        .to_string();
    let display_name = if relative.is_empty() {
        project.name.clone()
    } else {
        relative
    };

    let result = match project.package_manager {
        PackageManager::Npm => pm_npm::Npm.update_packages(&project.path).await,
        PackageManager::Pnpm => pm_pnpm::Pnpm.update_packages(&project.path).await,
        PackageManager::Yarn => pm_yarn::Yarn.update_packages(&project.path).await,
        PackageManager::Bun => pm_bun::Bun.update_packages(&project.path).await,
    };

    match result {
        Ok(output) => NpmUpdateResult {
            project_name: display_name,
            package_manager: project.package_manager,
            success: true,
            message: output,
        },
        Err(e) => NpmUpdateResult {
            project_name: display_name,
            package_manager: project.package_manager,
            success: false,
            message: e.to_string(),
        },
    }
}
