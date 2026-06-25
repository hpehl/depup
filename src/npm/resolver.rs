//! Resolves npm dependency versions against their package manager registries.
//!
//! Dispatches to the appropriate package manager implementation, runs
//! `list` and `outdated` commands concurrently via `tokio::try_join!`,
//! and merges the results into `VersionResult` values.

use std::path::Path;

use anyhow::Result;

use super::discovery::NpmProject;
use super::pm_version_check;
use crate::model::{CheckResult, CommandResult, Dependency, DependencyKind, Ecosystem};
use crate::version;

/// Resolves versions for a single npm project.
/// Dispatches to the detected package manager and merges installed + outdated data.
pub async fn resolve_project(project: &NpmProject, root: &Path) -> Result<Vec<CheckResult>> {
    let source = project.relative_source(root);

    // Tolerate missing PM binary: package queries need the CLI tool, but
    // the packageManager version check only needs HTTP.  When the binary is
    // absent (e.g. in CI), we still want tool-version results.
    let mut results = match project.package_manager.run_queries(&project.path).await {
        Ok((installed, outdated)) => merge_results(installed, &outdated, &source),
        Err(_) => Vec::new(),
    };

    if let Some(pm_result) = pm_version_check::check_pm_version(project, &source).await {
        results.push(pm_result);
    }

    results.sort_by(|a, b| a.artifact().cmp(b.artifact()));
    Ok(results)
}

fn merge_results(
    installed: Vec<super::InstalledPackage>,
    outdated: &std::collections::HashMap<String, super::OutdatedEntry>,
    source: &str,
) -> Vec<CheckResult> {
    installed
        .into_iter()
        .map(|pkg| {
            let kind = if pkg.is_dev {
                DependencyKind::NpmDevDep
            } else {
                DependencyKind::NpmDep
            };
            let id = Dependency::new(
                Ecosystem::Npm,
                kind,
                pkg.name.clone(),
                None,
                source.to_string(),
            );
            if let Some(entry) = outdated.get(&id.artifact) {
                let current = if entry.current.is_empty() {
                    &pkg.version
                } else {
                    &entry.current
                };
                let is_outdated = version::is_newer(current, &entry.latest);
                CheckResult::checked(id, current.clone(), entry.latest.clone(), is_outdated)
            } else {
                CheckResult::checked(id, pkg.version.clone(), pkg.version, false)
            }
        })
        .collect()
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

        let results = merge_results(installed, &outdated, "package.json");
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

        let results = merge_results(installed, &outdated, "package.json");
        assert!(results.is_empty());
    }

    #[test]
    fn outdated_with_empty_current_falls_back_to_installed_version() {
        let installed = vec![make_installed("react", "18.0.0", false)];
        let mut outdated = HashMap::new();
        outdated.insert(
            "react".to_string(),
            super::super::OutdatedEntry {
                current: String::new(),
                latest: "19.0.0".into(),
            },
        );

        let results = merge_results(installed, &outdated, "package.json");
        assert_eq!(results.len(), 1);

        let react = &results[0];
        assert_eq!(react.current_version, "18.0.0");
        assert!(react.is_outdated());
        if let CheckStatus::Outdated { latest } = &react.status {
            assert_eq!(latest, "19.0.0");
        }
    }

    #[test]
    fn dev_deps_classified_correctly() {
        let installed = vec![
            make_installed("react", "18.0.0", false),
            make_installed("jest", "29.0.0", true),
        ];
        let outdated: HashMap<String, super::super::OutdatedEntry> = HashMap::new();

        let results = merge_results(installed, &outdated, "package.json");
        assert_eq!(results[0].kind(), DependencyKind::NpmDep);
        assert_eq!(results[1].kind(), DependencyKind::NpmDevDep);
    }
}
