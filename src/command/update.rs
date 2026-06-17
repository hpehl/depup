//! The `update` subcommand: updates outdated dependencies in place.
//!
//! For Maven, rewrites version values in POM files preserving formatting.
//! For npm, delegates to the detected package manager's native update command.
//! Mirrors the check command's output style: grouped by ecosystem and kind,
//! with summary line, timing, and exit code.

use std::sync::Arc;

use anyhow::Result;
use clap::ArgMatches;
use console::style;
use indicatif::ProgressBar;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::Instant;

use crate::constants::MAX_CONCURRENT_REQUESTS;
use crate::model::{CheckResult, CommandResult, Ecosystem, UpdateResult};
use crate::output;
use crate::output::json::UpdateJsonResult;
use crate::progress;

/// Returns `true` if the process should exit with code 1 (update errors occurred).
pub async fn update(matches: &ArgMatches) -> Result<bool> {
    let setup = super::pipeline::CommandSetup::from_matches(matches);
    let dry_run = matches.get_flag("dry-run");
    let instant = Instant::now();

    // Phase 1: Check for outdated dependencies
    let pipeline = crate::command::pipeline::resolve_versions(&setup.resolve_config()).await?;

    // Filter to outdated results matching the user's filters
    let outdated: Vec<CheckResult> = pipeline
        .results
        .into_iter()
        .filter(|r| r.is_outdated() && setup.filter.matches(r))
        .collect();

    if outdated.is_empty() {
        if setup.json {
            println!("[]");
        } else {
            println!("{}", style("All dependencies are up to date.").green());
        }
        return Ok(false);
    }

    if dry_run {
        if setup.json {
            let json_results: Vec<UpdateJsonResult> = outdated
                .iter()
                .map(UpdateJsonResult::would_update)
                .collect();
            output::print_json(&json_results);
        } else {
            println!();
            println!();
            println!("{}", style("Dry run \u{2014} no changes made:").bold());
            let preview: Vec<UpdateResult> = outdated
                .iter()
                .map(|r| UpdateResult::updated(r, r.latest_version().unwrap_or("?").to_string()))
                .collect();
            output::print_table(&preview, "", output::update_summary);
            progress::done(instant);
        }
        return Ok(false);
    }

    // Phase 2: Apply updates
    let mut all_results: Vec<UpdateResult> = Vec::new();

    let maven_outdated: Vec<CheckResult> = outdated
        .iter()
        .filter(|r| r.ecosystem() == Ecosystem::Maven)
        .cloned()
        .collect();
    let npm_outdated: Vec<&CheckResult> = outdated
        .iter()
        .filter(|r| r.ecosystem() == Ecosystem::Npm)
        .collect();

    // Single progress bar for all updates (Maven POM count + npm project count)
    let maven_pom_count = maven_outdated
        .iter()
        .map(|r| r.source())
        .collect::<std::collections::HashSet<_>>()
        .len();
    let npm_project_count =
        count_npm_projects_with_outdated(&pipeline.npm_projects, &setup.root, &npm_outdated);
    let total = maven_pom_count + npm_project_count;
    let bar = progress::phase_bar("Updating", total as u64, setup.json);

    // Maven updates
    if !maven_outdated.is_empty() {
        let maven_results =
            crate::maven::updater::apply_updates(&setup.root, &maven_outdated, &bar)?;
        all_results.extend(maven_results);
    }

    // npm updates
    if !npm_outdated.is_empty() {
        let npm_results =
            run_npm_updates(&pipeline.npm_projects, &setup.root, &npm_outdated, &bar).await;
        all_results.extend(npm_results);
    }

    bar.finish_with_message("done");

    if setup.json {
        let json_results: Vec<UpdateJsonResult> =
            all_results.iter().map(UpdateJsonResult::from).collect();
        output::print_json(&json_results);
    } else {
        println!();
        println!();
        output::print_table(&all_results, "", output::update_summary);
        progress::done(instant);
    }

    Ok(all_results.iter().any(|r| r.is_error()))
}

/// Matches outdated results to their npm projects by source path.
fn match_npm_projects<'a>(
    projects: &'a [crate::npm::discovery::NpmProject],
    root: &std::path::Path,
    outdated: &[&CheckResult],
) -> Vec<(&'a crate::npm::discovery::NpmProject, Vec<CheckResult>)> {
    projects
        .iter()
        .filter_map(|p| {
            let project_source = p.relative_source(root);
            let project_results: Vec<CheckResult> = outdated
                .iter()
                .filter(|r| r.source() == project_source)
                .map(|r| (*r).clone())
                .collect();
            if project_results.is_empty() {
                None
            } else {
                Some((p, project_results))
            }
        })
        .collect()
}

fn count_npm_projects_with_outdated(
    projects: &[crate::npm::discovery::NpmProject],
    root: &std::path::Path,
    outdated: &[&CheckResult],
) -> usize {
    match_npm_projects(projects, root, outdated).len()
}

async fn run_npm_updates(
    projects: &[crate::npm::discovery::NpmProject],
    root: &std::path::Path,
    outdated: &[&CheckResult],
    bar: &ProgressBar,
) -> Vec<UpdateResult> {
    let projects_with_outdated = match_npm_projects(projects, root, outdated);

    if projects_with_outdated.is_empty() {
        return Vec::new();
    }

    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS));
    let mut join_set = JoinSet::new();

    for (project, project_results) in projects_with_outdated {
        let project = project.clone();
        let root = root.to_path_buf();
        let semaphore = Arc::clone(&semaphore);
        let bar = bar.clone();
        join_set.spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            bar.set_message(format!("{} ({})", project.name, project.package_manager));
            let result =
                crate::npm::updater::update_project(&project, &root, &project_results).await;
            bar.inc(1);
            result
        });
    }

    join_set.join_all().await.into_iter().flatten().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Dependency, DependencyKind, Ecosystem};
    use crate::npm::PackageManager;
    use crate::npm::discovery::NpmProject;
    use std::path::PathBuf;

    fn npm_result(name: &str, source: &str) -> CheckResult {
        CheckResult::checked(
            Dependency::new(
                Ecosystem::Npm,
                DependencyKind::NpmDep,
                name.into(),
                None,
                source.into(),
            ),
            "1.0.0".into(),
            "2.0.0".into(),
            true,
        )
    }

    fn npm_project(root: &std::path::Path, subdir: &str) -> NpmProject {
        NpmProject {
            name: subdir.to_string(),
            path: root.join(subdir),
            package_manager: PackageManager::Npm,
            pm_version: None,
        }
    }

    #[test]
    fn matches_outdated_to_correct_project() {
        let root = PathBuf::from("/repo");
        let projects = vec![npm_project(&root, "app-a"), npm_project(&root, "app-b")];
        let r1 = npm_result("react", "app-a/package.json");
        let r2 = npm_result("lodash", "app-b/package.json");
        let outdated: Vec<&CheckResult> = vec![&r1, &r2];

        let matched = match_npm_projects(&projects, &root, &outdated);
        assert_eq!(matched.len(), 2);
        assert_eq!(matched[0].0.name, "app-a");
        assert_eq!(matched[0].1.len(), 1);
        assert_eq!(matched[1].0.name, "app-b");
        assert_eq!(matched[1].1.len(), 1);
    }

    #[test]
    fn skips_projects_without_outdated() {
        let root = PathBuf::from("/repo");
        let projects = vec![npm_project(&root, "app-a"), npm_project(&root, "app-b")];
        let r1 = npm_result("react", "app-a/package.json");
        let outdated: Vec<&CheckResult> = vec![&r1];

        let matched = match_npm_projects(&projects, &root, &outdated);
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].0.name, "app-a");
    }

    #[test]
    fn empty_outdated_returns_empty() {
        let root = PathBuf::from("/repo");
        let projects = vec![npm_project(&root, "app-a")];
        let outdated: Vec<&CheckResult> = vec![];

        let matched = match_npm_projects(&projects, &root, &outdated);
        assert!(matched.is_empty());
    }

    #[test]
    fn count_matches_len() {
        let root = PathBuf::from("/repo");
        let projects = vec![npm_project(&root, "app-a"), npm_project(&root, "app-b")];
        let r1 = npm_result("react", "app-a/package.json");
        let outdated: Vec<&CheckResult> = vec![&r1];

        assert_eq!(
            count_npm_projects_with_outdated(&projects, &root, &outdated),
            1
        );
    }
}
