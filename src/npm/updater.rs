//! Orchestrates npm ecosystem updates by delegating to each project's
//! package manager native update command and rewriting the `packageManager`
//! field in `package.json` when the PM version itself is outdated.

use std::path::Path;

use super::discovery::NpmProject;
use super::pm_version_check;
use crate::model::{CheckResult, CommandResult, DependencyKind, UpdateResult};

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

    // Update regular package dependencies via targeted installs
    if !package_deps.is_empty() {
        let packages: Vec<(&str, &str, bool)> = package_deps
            .iter()
            .filter_map(|r| {
                let version = r.latest_version()?;
                let is_dev = r.kind() == DependencyKind::NpmDevDep;
                Some((r.artifact(), version, is_dev))
            })
            .collect();

        let result = project
            .package_manager
            .update(&project.path, &packages)
            .await;

        match result {
            Ok(()) => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CheckStatus, Dependency, Ecosystem};
    use crate::npm::PackageManager;

    fn outdated_check(
        name: &str,
        current: &str,
        latest: &str,
        kind: DependencyKind,
    ) -> CheckResult {
        CheckResult {
            dep: Dependency::new(
                Ecosystem::Npm,
                kind,
                name.into(),
                None,
                "package.json".into(),
            ),
            current_version: current.into(),
            status: CheckStatus::Outdated {
                latest: latest.into(),
            },
        }
    }

    #[test]
    fn partitions_tool_versions_from_packages() {
        let outdated = vec![
            outdated_check("react", "18.0.0", "19.0.0", DependencyKind::NpmDep),
            outdated_check("npm", "9.0.0", "10.0.0", DependencyKind::Tool),
            outdated_check("lodash", "4.0.0", "5.0.0", DependencyKind::NpmDep),
        ];
        let (tools, packages): (Vec<_>, Vec<_>) = outdated
            .iter()
            .partition(|r| r.kind() == DependencyKind::Tool);
        assert_eq!(tools.len(), 1);
        assert_eq!(packages.len(), 2);
    }

    #[test]
    fn tool_version_with_no_latest_produces_error() {
        let project = NpmProject {
            path: std::path::PathBuf::from("/tmp/test"),
            name: "test".into(),
            package_manager: PackageManager::Npm,
            pm_version: None,
        };

        let check = CheckResult {
            dep: Dependency::new(
                Ecosystem::Npm,
                DependencyKind::Tool,
                "npm".into(),
                None,
                "package.json".into(),
            ),
            current_version: "9.0.0".into(),
            status: CheckStatus::Outdated {
                latest: String::new(),
            },
        };

        let rt = tokio::runtime::Runtime::new().unwrap();
        let results = rt.block_on(update_project(
            &project,
            std::path::Path::new("/tmp"),
            &[check],
        ));
        assert_eq!(results.len(), 1);
        assert!(results[0].is_error());
    }

    #[test]
    fn empty_outdated_returns_empty_results() {
        let project = NpmProject {
            path: std::path::PathBuf::from("/tmp/test"),
            name: "test".into(),
            package_manager: PackageManager::Npm,
            pm_version: None,
        };

        let rt = tokio::runtime::Runtime::new().unwrap();
        let results = rt.block_on(update_project(&project, std::path::Path::new("/tmp"), &[]));
        assert!(results.is_empty());
    }
}
