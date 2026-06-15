pub mod discovery;
mod pm_bun;
mod pm_npm;
mod pm_pnpm;
mod pm_yarn;

use std::collections::HashMap;
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

pub async fn check_project(project: &NpmProject, root: &Path) -> Result<Vec<CheckResult>> {
    let source = project
        .path
        .strip_prefix(root)
        .unwrap_or(&project.path)
        .join("package.json")
        .display()
        .to_string();

    let (installed, outdated) = match project.package_manager {
        PackageManager::Npm => {
            let checker = pm_npm::Npm;
            tokio::try_join!(
                checker.list_packages(&project.path),
                checker.outdated_packages(&project.path),
            )?
        }
        PackageManager::Pnpm => {
            let checker = pm_pnpm::Pnpm;
            tokio::try_join!(
                checker.list_packages(&project.path),
                checker.outdated_packages(&project.path),
            )?
        }
        PackageManager::Yarn => {
            let checker = pm_yarn::Yarn;
            tokio::try_join!(
                checker.list_packages(&project.path),
                checker.outdated_packages(&project.path),
            )?
        }
        PackageManager::Bun => {
            let checker = pm_bun::Bun;
            tokio::try_join!(
                checker.list_packages(&project.path),
                checker.outdated_packages(&project.path),
            )?
        }
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
                )
            }
            .with_source(source.clone())
        })
        .collect();

    results.sort_by(|a, b| a.property_name.cmp(&b.property_name));
    Ok(results)
}
