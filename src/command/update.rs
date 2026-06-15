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

use crate::app;
use crate::constants::MAX_CONCURRENT_REQUESTS;
use crate::dependency::{Ecosystem, UpdateResult, VersionResult};
use crate::filter::Filter;
use crate::json::UpdateJsonResult;
use crate::output;
use crate::progress;

pub async fn update(matches: &ArgMatches) -> Result<()> {
    let path = app::path_argument(matches);
    let json = app::is_json(matches);
    let dry_run = matches.get_flag("dry-run");
    let filter = Filter::from_matches(matches);

    let instant = Instant::now();
    let root = path.canonicalize().unwrap_or_else(|_| path.clone());

    let (do_maven, do_npm) = super::pipeline::detect_ecosystems(&filter, &root);

    // Phase 1: Check for outdated dependencies
    let (check_results, npm_projects) =
        crate::command::pipeline::resolve_versions(&root, do_maven, do_npm, filter.stable, json)
            .await?;

    // Filter to outdated results matching the user's filters
    let outdated: Vec<VersionResult> = check_results
        .into_iter()
        .filter(|r| r.is_outdated() && filter.matches(r))
        .collect();

    if outdated.is_empty() {
        if json {
            println!("[]");
        } else {
            println!("{}", style("All dependencies are up to date.").green());
        }
        return Ok(());
    }

    if dry_run {
        if json {
            let json_results: Vec<UpdateJsonResult> = outdated
                .iter()
                .map(UpdateJsonResult::would_update)
                .collect();
            output::print_update_json(&json_results);
        } else {
            println!();
            println!("{}", style("Dry run \u{2014} no changes made:").bold());
            let preview: Vec<UpdateResult> = outdated
                .iter()
                .map(|r| UpdateResult::updated(r, r.latest_version().unwrap_or("?").to_string()))
                .collect();
            output::print_update_results(&preview);
            progress::done(instant);
        }
        return Ok(());
    }

    // Phase 2: Apply updates
    let mut all_results: Vec<UpdateResult> = Vec::new();

    let maven_outdated: Vec<VersionResult> = outdated
        .iter()
        .filter(|r| r.ecosystem() == Ecosystem::Maven)
        .cloned()
        .collect();
    let npm_outdated: Vec<&VersionResult> = outdated
        .iter()
        .filter(|r| r.ecosystem() == Ecosystem::Npm)
        .collect();

    // Single progress bar for all updates (Maven POM count + npm project count)
    let maven_pom_count = maven_outdated
        .iter()
        .map(|r| r.source())
        .collect::<std::collections::HashSet<_>>()
        .len();
    let npm_project_count = count_npm_projects_with_outdated(&npm_projects, &root, &npm_outdated);
    let total = maven_pom_count + npm_project_count;
    let bar = if json || total == 0 {
        ProgressBar::hidden()
    } else {
        progress::bar(total as u64)
    };

    // Maven updates
    if !maven_outdated.is_empty() {
        let maven_results = crate::maven::updater::apply_updates(&root, &maven_outdated, &bar)?;
        all_results.extend(maven_results);
    }

    // npm updates
    if !npm_outdated.is_empty() {
        let npm_results = run_npm_updates(&npm_projects, &root, &npm_outdated, &bar).await;
        all_results.extend(npm_results);
    }

    bar.finish_and_clear();

    if json {
        let json_results: Vec<UpdateJsonResult> =
            all_results.iter().map(UpdateJsonResult::from).collect();
        output::print_update_json(&json_results);
    } else {
        println!();
        output::print_update_results(&all_results);
        progress::done(instant);
    }

    if all_results.iter().any(|r| r.is_error()) {
        std::process::exit(1);
    }

    Ok(())
}

/// Matches outdated results to their npm projects by source path.
fn match_npm_projects<'a>(
    projects: &'a [crate::npm::discovery::NpmProject],
    root: &std::path::Path,
    outdated: &[&VersionResult],
) -> Vec<(&'a crate::npm::discovery::NpmProject, Vec<VersionResult>)> {
    projects
        .iter()
        .filter_map(|p| {
            let project_source = p
                .path
                .strip_prefix(root)
                .unwrap_or(&p.path)
                .join("package.json")
                .display()
                .to_string();
            let project_results: Vec<VersionResult> = outdated
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
    outdated: &[&VersionResult],
) -> usize {
    match_npm_projects(projects, root, outdated).len()
}

async fn run_npm_updates(
    projects: &[crate::npm::discovery::NpmProject],
    root: &std::path::Path,
    outdated: &[&VersionResult],
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
