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
    Vec<super::InstalledPackage>,
    std::collections::HashMap<String, super::OutdatedEntry>,
)> {
    tokio::try_join!(resolver.list_packages(dir), resolver.outdated_packages(dir))
}

/// Resolves versions for a single npm project.
/// Dispatches to the detected package manager and merges installed + outdated data.
pub async fn resolve_project(project: &NpmProject, root: &Path) -> Result<Vec<CheckResult>> {
    let source = project.relative_source(root);

    let (installed, outdated) = match project.package_manager {
        PackageManager::Npm => run_queries(&pm_npm::Npm, &project.path).await?,
        PackageManager::Pnpm => run_queries(&pm_pnpm::Pnpm, &project.path).await?,
        PackageManager::Yarn => run_queries(&pm_yarn::Yarn, &project.path).await?,
        PackageManager::Bun => run_queries(&pm_bun::Bun, &project.path).await?,
    };

    let mut results: Vec<CheckResult> = installed
        .into_iter()
        .map(|pkg| {
            let kind = if pkg.is_dev {
                DependencyKind::NpmDevDep
            } else {
                DependencyKind::NpmDep
            };
            let id = Dependency::new(Ecosystem::Npm, kind, pkg.name.clone(), None, source.clone());
            if let Some(entry) = outdated.get(&id.artifact) {
                let is_outdated = version::is_newer(&entry.current, &entry.latest);
                CheckResult::checked(id, entry.current.clone(), entry.latest.clone(), is_outdated)
            } else {
                CheckResult::checked(id, pkg.version.clone(), pkg.version, false)
            }
        })
        .collect();

    if let Some(pm_result) = pm_version_check::check_pm_version(project, &source).await {
        results.push(pm_result);
    }

    results.sort_by(|a, b| a.artifact().cmp(b.artifact()));
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::CheckStatus;
    use std::collections::HashMap;

    fn make_installed(name: &str, version: &str, is_dev: bool) -> super::super::InstalledPackage {
        super::super::InstalledPackage {
            name: name.into(),
            version: version.into(),
            is_dev,
        }
    }

    #[test]
    fn merge_installed_with_outdated() {
        let installed = vec![
            make_installed("react", "18.0.0", false),
            make_installed("lodash", "4.17.21", false),
            make_installed("vitest", "1.0.0", true),
        ];
        let mut outdated = HashMap::new();
        outdated.insert(
            "react".to_string(),
            super::super::OutdatedEntry {
                current: "18.0.0".into(),
                latest: "19.0.0".into(),
            },
        );

        let source = "package.json".to_string();
        let results: Vec<CheckResult> = installed
            .into_iter()
            .map(|pkg| {
                let kind = if pkg.is_dev {
                    DependencyKind::NpmDevDep
                } else {
                    DependencyKind::NpmDep
                };
                let id =
                    Dependency::new(Ecosystem::Npm, kind, pkg.name.clone(), None, source.clone());
                if let Some(entry) = outdated.get(&id.artifact) {
                    let is_outdated = version::is_newer(&entry.current, &entry.latest);
                    CheckResult::checked(
                        id,
                        entry.current.clone(),
                        entry.latest.clone(),
                        is_outdated,
                    )
                } else {
                    CheckResult::checked(id, pkg.version.clone(), pkg.version, false)
                }
            })
            .collect();

        assert_eq!(results.len(), 3);

        let react = results.iter().find(|r| r.artifact() == "react").unwrap();
        assert!(react.is_outdated());
        assert_eq!(react.current_version, "18.0.0");
        if let CheckStatus::Outdated { latest } = &react.status {
            assert_eq!(latest, "19.0.0");
        }

        let lodash = results.iter().find(|r| r.artifact() == "lodash").unwrap();
        assert!(!lodash.is_outdated());

        let vitest = results.iter().find(|r| r.artifact() == "vitest").unwrap();
        assert_eq!(vitest.kind(), DependencyKind::NpmDevDep);
    }

    #[test]
    fn empty_installed_returns_empty() {
        let installed: Vec<super::super::InstalledPackage> = Vec::new();
        let outdated: HashMap<String, super::super::OutdatedEntry> = HashMap::new();
        let source = "package.json".to_string();

        let results: Vec<CheckResult> = installed
            .into_iter()
            .map(|pkg| {
                let kind = if pkg.is_dev {
                    DependencyKind::NpmDevDep
                } else {
                    DependencyKind::NpmDep
                };
                let id =
                    Dependency::new(Ecosystem::Npm, kind, pkg.name.clone(), None, source.clone());
                if let Some(entry) = outdated.get(&id.artifact) {
                    CheckResult::checked(id, entry.current.clone(), entry.latest.clone(), true)
                } else {
                    CheckResult::checked(id, pkg.version.clone(), pkg.version, false)
                }
            })
            .collect();

        assert!(results.is_empty());
    }

    #[test]
    fn dev_deps_classified_correctly() {
        let installed = vec![
            make_installed("react", "18.0.0", false),
            make_installed("jest", "29.0.0", true),
        ];
        let source = "package.json".to_string();

        let results: Vec<CheckResult> = installed
            .into_iter()
            .map(|pkg| {
                let kind = if pkg.is_dev {
                    DependencyKind::NpmDevDep
                } else {
                    DependencyKind::NpmDep
                };
                let id =
                    Dependency::new(Ecosystem::Npm, kind, pkg.name.clone(), None, source.clone());
                CheckResult::checked(id, pkg.version.clone(), pkg.version, false)
            })
            .collect();

        assert_eq!(results[0].kind(), DependencyKind::NpmDep);
        assert_eq!(results[1].kind(), DependencyKind::NpmDevDep);
    }
}
