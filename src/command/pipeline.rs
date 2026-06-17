//! Shared discovery and version resolution pipeline used by `check`, `update`, and `audit`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use clap::ArgMatches;
use indicatif::ProgressBar;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::app;
use crate::constants::MAX_CONCURRENT_REQUESTS;
use crate::filter::Filter;
use crate::model::{CheckResult, Dependency, DependencyKind, Ecosystem};
use crate::npm::discovery::NpmProject;

/// Common setup extracted from CLI arguments, shared by check, update, and audit.
pub struct CommandSetup {
    pub root: PathBuf,
    pub ecosystems: EcosystemSelection,
    pub filter: Filter,
    pub json: bool,
}

impl CommandSetup {
    pub fn from_matches(matches: &ArgMatches) -> Self {
        let path = app::path_argument(matches);
        let json = app::is_json(matches);
        let filter = Filter::from_matches(matches);
        let root = path.canonicalize().unwrap_or_else(|_| path.clone());
        let ecosystems = detect_ecosystems(&filter, &root);
        Self {
            root,
            ecosystems,
            filter,
            json,
        }
    }

    pub fn resolve_config(&self) -> ResolveConfig<'_> {
        ResolveConfig {
            root: &self.root,
            ecosystems: &self.ecosystems,
            stable: self.filter.stable,
            json: self.json,
        }
    }
}

/// Which ecosystems to discover and check.
pub struct EcosystemSelection {
    pub maven: bool,
    pub npm: bool,
}

/// Determines which ecosystems to discover based on filters and project files.
pub fn detect_ecosystems(filter: &Filter, root: &Path) -> EcosystemSelection {
    EcosystemSelection {
        maven: filter.ecosystem.is_none_or(|e| e != Ecosystem::Npm)
            && root.join("pom.xml").exists(),
        npm: filter.ecosystem.is_none_or(|e| e != Ecosystem::Maven),
    }
}

/// Configuration for the shared version resolution pipeline.
pub struct ResolveConfig<'a> {
    pub root: &'a Path,
    pub ecosystems: &'a EcosystemSelection,
    pub stable: bool,
    pub json: bool,
}

/// Results from the shared version resolution pipeline.
pub struct PipelineResult {
    pub results: Vec<CheckResult>,
    pub(crate) npm_projects: Vec<NpmProject>,
}

/// Discovers dependencies and resolves their versions across all ecosystems.
pub async fn resolve_versions(config: &ResolveConfig<'_>) -> Result<PipelineResult> {
    let maven_prepared = if config.ecosystems.maven {
        Some(crate::maven::resolver::discover(
            config.root,
            config.stable,
        )?)
    } else {
        None
    };
    let npm_projects = if config.ecosystems.npm {
        crate::npm::discovery::discover(config.root)
    } else {
        Vec::new()
    };

    let maven_count = maven_prepared.as_ref().map_or(0, |p| p.count());
    let npm_count = npm_projects.len();
    let total = maven_count + npm_count;

    if total == 0 {
        return Ok(PipelineResult {
            results: Vec::new(),
            npm_projects,
        });
    }

    let bar = crate::progress::phase_bar("Collecting", total as u64, config.json);

    let mut join_set: JoinSet<Vec<CheckResult>> = JoinSet::new();

    if let Some(prepared) = maven_prepared {
        let root = config.root.to_path_buf();
        let bar = bar.clone();
        join_set.spawn(async move { crate::maven::resolver::resolve(&root, prepared, &bar).await });
    }

    spawn_npm_resolves(&mut join_set, &npm_projects, config.root, &bar);

    let results: Vec<CheckResult> = join_set.join_all().await.into_iter().flatten().collect();
    bar.finish_with_message("done");

    Ok(PipelineResult {
        results,
        npm_projects,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn detect_both_when_pom_exists_and_no_filter() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("pom.xml"), "<project/>").unwrap();
        let eco = detect_ecosystems(&Filter::default(), tmp.path());
        assert!(eco.maven);
        assert!(eco.npm);
    }

    #[test]
    fn detect_npm_only_when_no_pom() {
        let tmp = TempDir::new().unwrap();
        let eco = detect_ecosystems(&Filter::default(), tmp.path());
        assert!(!eco.maven);
        assert!(eco.npm);
    }

    #[test]
    fn detect_maven_only_with_maven_filter() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("pom.xml"), "<project/>").unwrap();
        let filter = Filter {
            ecosystem: Some(Ecosystem::Maven),
            ..Filter::default()
        };
        let eco = detect_ecosystems(&filter, tmp.path());
        assert!(eco.maven);
        assert!(!eco.npm);
    }

    #[test]
    fn detect_npm_only_with_npm_filter() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("pom.xml"), "<project/>").unwrap();
        let filter = Filter {
            ecosystem: Some(Ecosystem::Npm),
            ..Filter::default()
        };
        let eco = detect_ecosystems(&filter, tmp.path());
        assert!(!eco.maven);
        assert!(eco.npm);
    }

    #[test]
    fn detect_nothing_when_npm_filter_and_no_pom() {
        let tmp = TempDir::new().unwrap();
        let filter = Filter {
            ecosystem: Some(Ecosystem::Npm),
            ..Filter::default()
        };
        let eco = detect_ecosystems(&filter, tmp.path());
        assert!(!eco.maven);
        assert!(eco.npm);
    }
}

/// Spawns npm project version resolution concurrently with semaphore-based rate limiting.
/// On failure, produces an error `VersionResult` rather than propagating the error.
fn spawn_npm_resolves(
    join_set: &mut JoinSet<Vec<CheckResult>>,
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
            let results = crate::npm::resolver::resolve_project(&project, &root)
                .await
                .unwrap_or_else(|e| {
                    let source = project.relative_source(&root);
                    let id = Dependency::new(
                        Ecosystem::Npm,
                        DependencyKind::NpmDep,
                        project.name.clone(),
                        None,
                        source,
                    );
                    vec![CheckResult::error(id, String::new(), e.to_string())]
                });
            bar.inc(1);
            results
        });
    }
}
