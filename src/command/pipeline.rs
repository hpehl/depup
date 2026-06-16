//! Shared discovery and version resolution pipeline used by `check`, `update`, and `audit`.

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use indicatif::ProgressBar;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::constants::MAX_CONCURRENT_REQUESTS;
use crate::dependency::{Dependency, DependencyKind, Ecosystem, VersionResult};
use crate::filter::Filter;
use crate::npm::discovery::NpmProject;
use crate::progress;

/// Determines which ecosystems to discover based on filters and project files.
pub fn detect_ecosystems(filter: &Filter, root: &Path) -> (bool, bool) {
    let do_maven =
        filter.ecosystem.is_none_or(|e| e != Ecosystem::Npm) && root.join("pom.xml").exists();
    let do_npm = filter.ecosystem.is_none_or(|e| e != Ecosystem::Maven);
    (do_maven, do_npm)
}

/// Discovers dependencies and resolves their versions across all ecosystems.
///
/// Returns the version results and the discovered npm projects (needed by update
/// to delegate to the correct package manager).
pub async fn resolve_versions(
    root: &Path,
    do_maven: bool,
    do_npm: bool,
    stable: bool,
    json: bool,
) -> Result<(Vec<VersionResult>, Vec<NpmProject>)> {
    let maven_prepared = if do_maven {
        Some(crate::maven::resolver::discover(root, stable)?)
    } else {
        None
    };
    let npm_projects = if do_npm {
        crate::npm::discovery::discover(root)
    } else {
        Vec::new()
    };

    let maven_count = maven_prepared.as_ref().map_or(0, |p| p.count());
    let npm_count = npm_projects.len();
    let total = maven_count + npm_count;

    if total == 0 {
        return Ok((Vec::new(), npm_projects));
    }

    let bar = if json {
        ProgressBar::hidden()
    } else {
        progress::bar(total as u64)
    };

    let mut join_set: JoinSet<Vec<VersionResult>> = JoinSet::new();

    if let Some(prepared) = maven_prepared {
        let root = root.to_path_buf();
        let bar = bar.clone();
        join_set.spawn(async move { crate::maven::resolver::resolve(&root, prepared, &bar).await });
    }

    spawn_npm_resolves(&mut join_set, &npm_projects, root, &bar);

    let results: Vec<VersionResult> = join_set.join_all().await.into_iter().flatten().collect();
    bar.finish_and_clear();

    Ok((results, npm_projects))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn detect_both_when_pom_exists_and_no_filter() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("pom.xml"), "<project/>").unwrap();
        let (maven, npm) = detect_ecosystems(&Filter::default(), tmp.path());
        assert!(maven);
        assert!(npm);
    }

    #[test]
    fn detect_npm_only_when_no_pom() {
        let tmp = TempDir::new().unwrap();
        let (maven, npm) = detect_ecosystems(&Filter::default(), tmp.path());
        assert!(!maven);
        assert!(npm);
    }

    #[test]
    fn detect_maven_only_with_maven_filter() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("pom.xml"), "<project/>").unwrap();
        let filter = Filter {
            ecosystem: Some(Ecosystem::Maven),
            ..Filter::default()
        };
        let (maven, npm) = detect_ecosystems(&filter, tmp.path());
        assert!(maven);
        assert!(!npm);
    }

    #[test]
    fn detect_npm_only_with_npm_filter() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("pom.xml"), "<project/>").unwrap();
        let filter = Filter {
            ecosystem: Some(Ecosystem::Npm),
            ..Filter::default()
        };
        let (maven, npm) = detect_ecosystems(&filter, tmp.path());
        assert!(!maven);
        assert!(npm);
    }

    #[test]
    fn detect_nothing_when_npm_filter_and_no_pom() {
        let tmp = TempDir::new().unwrap();
        let filter = Filter {
            ecosystem: Some(Ecosystem::Npm),
            ..Filter::default()
        };
        let (maven, npm) = detect_ecosystems(&filter, tmp.path());
        assert!(!maven);
        assert!(npm);
    }
}

/// Spawns npm project version resolution concurrently with semaphore-based rate limiting.
/// On failure, produces an error `VersionResult` rather than propagating the error.
fn spawn_npm_resolves(
    join_set: &mut JoinSet<Vec<VersionResult>>,
    projects: &[NpmProject],
    root: &Path,
    bar: &ProgressBar,
) {
    if projects.is_empty() {
        return;
    }

    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS));
    for project in projects {
        let project = project.clone();
        let semaphore = Arc::clone(&semaphore);
        let root = root.to_path_buf();
        let bar = bar.clone();
        join_set.spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            bar.set_message(format!("{} ({})", project.name, project.package_manager));
            let project_name = project.name.clone();
            let project_path = project.path.clone();
            let results = crate::npm::resolver::resolve_project(&project, &root)
                .await
                .unwrap_or_else(|e| {
                    let source = project_path
                        .strip_prefix(&root)
                        .unwrap_or(&project_path)
                        .join("package.json")
                        .display()
                        .to_string();
                    let id = Dependency::new(
                        Ecosystem::Npm,
                        DependencyKind::NpmDep,
                        project_name,
                        None,
                        source,
                    );
                    vec![VersionResult::error(id, String::new(), e.to_string())]
                });
            bar.inc(1);
            results
        });
    }
}
