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
use crate::filter::Filter;
use crate::json::UpdateJsonResult;
use crate::output;
use crate::progress;
use crate::registry::{CheckResult, Ecosystem, UpdateResult};

pub async fn update(matches: &ArgMatches) -> Result<()> {
    let path = app::path_argument(matches);
    let json = app::is_json(matches);
    let dry_run = matches.get_flag("dry-run");
    let filter = Filter::from_matches(matches);

    let instant = Instant::now();
    let root = path.canonicalize().unwrap_or_else(|_| path.clone());

    let do_maven =
        filter.ecosystem.is_none_or(|e| e != Ecosystem::Npm) && root.join("pom.xml").exists();
    let do_npm = filter.ecosystem.is_none_or(|e| e != Ecosystem::Maven);

    // Phase 1: Check for outdated dependencies
    let (check_results, npm_projects) =
        crate::command::pipeline::run_checks(&root, do_maven, do_npm, filter.stable, json).await?;

    // Filter to outdated results matching the user's filters
    let outdated: Vec<CheckResult> = check_results
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

    // Maven updates
    let maven_outdated: Vec<CheckResult> = outdated
        .iter()
        .filter(|r| r.ecosystem() == Ecosystem::Maven)
        .cloned()
        .collect();
    if !maven_outdated.is_empty() {
        let maven_results = crate::maven::updater::apply_updates(&root, &maven_outdated)?;
        all_results.extend(maven_results);
    }

    // npm updates -- only projects that have outdated deps
    let npm_outdated: Vec<&CheckResult> = outdated
        .iter()
        .filter(|r| r.ecosystem() == Ecosystem::Npm)
        .collect();
    if !npm_outdated.is_empty() {
        let npm_results = run_npm_updates(&npm_projects, &root, &npm_outdated, json).await;
        all_results.extend(npm_results);
    }

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

async fn run_npm_updates(
    projects: &[crate::npm::discovery::NpmProject],
    root: &std::path::Path,
    outdated: &[&CheckResult],
    json: bool,
) -> Vec<UpdateResult> {
    // Match outdated results to their npm projects by source path
    let projects_with_outdated: Vec<(&crate::npm::discovery::NpmProject, Vec<CheckResult>)> =
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
            .collect();

    if projects_with_outdated.is_empty() {
        return Vec::new();
    }

    let bar = if json {
        ProgressBar::hidden()
    } else {
        progress::bar(projects_with_outdated.len() as u64)
    };

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

    let results: Vec<UpdateResult> = join_set.join_all().await.into_iter().flatten().collect();
    bar.finish_and_clear();
    results
}
