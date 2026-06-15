//! Shared discovery and check pipeline used by both `check` and `update` commands.

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use indicatif::ProgressBar;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::constants::MAX_CONCURRENT_REQUESTS;
use crate::npm::discovery::NpmProject;
use crate::progress;
use crate::registry::{CheckId, CheckResult, CheckerKind, Ecosystem};

/// Discovers dependencies and runs checks across all ecosystems.
///
/// Returns the check results and the discovered npm projects (needed by update
/// to delegate to the correct package manager).
pub async fn run_checks(
    root: &Path,
    do_maven: bool,
    do_npm: bool,
    stable: bool,
    json: bool,
) -> Result<(Vec<CheckResult>, Vec<NpmProject>)> {
    let maven_prepared = if do_maven {
        Some(crate::maven::checker::discover(root, stable)?)
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

    let mut join_set: JoinSet<Vec<CheckResult>> = JoinSet::new();

    if let Some(prepared) = maven_prepared {
        let root = root.to_path_buf();
        let bar = bar.clone();
        join_set.spawn(async move { crate::maven::checker::check(&root, prepared, &bar).await });
    }

    spawn_npm_checks(&mut join_set, &npm_projects, root, &bar);

    let results: Vec<CheckResult> = join_set.join_all().await.into_iter().flatten().collect();
    bar.finish_and_clear();

    Ok((results, npm_projects))
}

/// Spawns npm project checks concurrently with semaphore-based rate limiting.
/// On failure, produces an error `CheckResult` rather than propagating the error.
fn spawn_npm_checks(
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
            let project_name = project.name.clone();
            let project_path = project.path.clone();
            let results = crate::npm::checker::check_project(&project, &root)
                .await
                .unwrap_or_else(|e| {
                    let source = project_path
                        .strip_prefix(&root)
                        .unwrap_or(&project_path)
                        .join("package.json")
                        .display()
                        .to_string();
                    let id = CheckId::new(
                        Ecosystem::Npm,
                        CheckerKind::NpmDep,
                        project_name,
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
