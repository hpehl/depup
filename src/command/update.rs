//! The `update` subcommand: updates outdated dependencies in place.
//!
//! For Maven, rewrites `<properties>` values in POM files preserving formatting.
//! For npm, delegates to the detected package manager's native update command.

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use clap::ArgMatches;
use console::style;
use indicatif::ProgressBar;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::app;
use crate::constants::MAX_CONCURRENT_REQUESTS;
use crate::npm::discovery::NpmProject;
use crate::progress;
use crate::registry::{CheckResult, Ecosystem};

pub async fn update(matches: &ArgMatches) -> Result<()> {
    let path = app::path_argument(matches);
    let json = app::is_json(matches);
    let stable = matches.get_flag("stable");
    let dry_run = matches.get_flag("dry-run");
    let maven_only = matches.get_flag("maven");
    let npm_only = matches.get_flag("npm");

    let root = path.canonicalize().unwrap_or_else(|_| path.clone());

    let do_maven = !npm_only && root.join("pom.xml").exists();
    let do_npm = !maven_only;

    // Phase 1: Check for outdated dependencies
    let check_results = run_checks(&root, do_maven, do_npm, stable, json).await?;

    let has_maven_outdated = check_results
        .iter()
        .any(|r| r.ecosystem() == Ecosystem::Maven && r.is_outdated());
    let npm_projects = if do_npm {
        crate::npm::discovery::discover(&root)
    } else {
        Vec::new()
    };
    let has_npm_outdated = !npm_projects.is_empty()
        && check_results
            .iter()
            .any(|r| r.ecosystem() == Ecosystem::Npm && r.is_outdated());

    if !has_maven_outdated && !has_npm_outdated {
        if json {
            println!("[]");
        } else {
            println!("{}", style("All dependencies are up to date.").green());
        }
        return Ok(());
    }

    if dry_run {
        print_dry_run(&check_results, json);
        return Ok(());
    }

    // Phase 2: Apply updates
    let mut json_results: Vec<serde_json::Value> = Vec::new();

    if do_maven && has_maven_outdated {
        let (updated, skipped) = crate::maven::updater::apply_updates(&root, &check_results)?;

        if json {
            for u in &updated {
                json_results.push(serde_json::json!({
                    "ecosystem": "maven",
                    "property": u.property,
                    "old_version": u.old_version,
                    "new_version": u.new_version,
                    "source": u.pom_path.strip_prefix(&root)
                        .unwrap_or(&u.pom_path).display().to_string(),
                    "status": "updated"
                }));
            }
            for s in &skipped {
                json_results.push(serde_json::json!({
                    "ecosystem": "maven",
                    "message": s,
                    "status": "skipped"
                }));
            }
        } else {
            if !updated.is_empty() {
                println!();
            }
            for u in &updated {
                println!(
                    "  {} {} {} {} {}",
                    style("\u{2713}").green().bold(),
                    style(&u.property).cyan(),
                    style(&u.old_version).dim(),
                    style("\u{2192}").yellow(),
                    style(&u.new_version).green()
                );
            }
            for s in &skipped {
                println!("  {} {}", style("-").dim(), style(s).dim());
            }
        }
    }

    if do_npm && has_npm_outdated {
        let npm_results = run_npm_updates(&npm_projects, &root, json).await;

        if json {
            for r in &npm_results {
                json_results.push(serde_json::json!({
                    "ecosystem": "npm",
                    "project": r.project_name,
                    "package_manager": r.package_manager.to_string(),
                    "status": if r.success { "updated" } else { "error" },
                    "message": r.message.trim()
                }));
            }
        } else {
            for r in &npm_results {
                if r.success {
                    println!(
                        "  {} {} ({})",
                        style("\u{2713}").green().bold(),
                        style(&r.project_name).blue(),
                        r.package_manager
                    );
                } else {
                    println!(
                        "  {} {} ({}): {}",
                        style("\u{2717}").red().bold(),
                        style(&r.project_name).blue(),
                        r.package_manager,
                        style(&r.message).red()
                    );
                }
            }
        }
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json_results).unwrap_or_else(|_| "[]".to_string())
        );
    }

    Ok(())
}

async fn run_checks(
    root: &Path,
    do_maven: bool,
    do_npm: bool,
    stable: bool,
    json: bool,
) -> Result<Vec<CheckResult>> {
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
        return Ok(Vec::new());
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

    crate::command::check::spawn_npm_checks(&mut join_set, npm_projects, root, &bar);

    let results: Vec<CheckResult> = join_set.join_all().await.into_iter().flatten().collect();
    bar.finish_and_clear();

    Ok(results)
}

async fn run_npm_updates(
    projects: &[NpmProject],
    root: &Path,
    json: bool,
) -> Vec<crate::npm::updater::NpmUpdateResult> {
    let bar = if json {
        ProgressBar::hidden()
    } else {
        progress::bar(projects.len() as u64)
    };

    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS));
    let mut join_set = JoinSet::new();

    for project in projects {
        let project = project.clone();
        let root = root.to_path_buf();
        let semaphore = Arc::clone(&semaphore);
        let bar = bar.clone();
        join_set.spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            bar.set_message(format!("{} ({})", project.name, project.package_manager));
            let result = crate::npm::updater::update_project(&project, &root).await;
            bar.inc(1);
            result
        });
    }

    let results = join_set.join_all().await;
    bar.finish_and_clear();
    results
}

fn print_dry_run(results: &[CheckResult], json: bool) {
    let outdated: Vec<&CheckResult> = results.iter().filter(|r| r.is_outdated()).collect();

    if json {
        let json_results: Vec<serde_json::Value> = outdated
            .iter()
            .map(|r| {
                serde_json::json!({
                    "ecosystem": r.ecosystem().to_string().to_lowercase(),
                    "property": r.property_name(),
                    "current": r.current_version,
                    "latest": r.latest_version().unwrap_or(""),
                    "status": "would_update"
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&json_results).unwrap_or_else(|_| "[]".to_string())
        );
    } else {
        println!("{}", style("Dry run — no changes made:").bold());
        for r in &outdated {
            let name = r.artifact().unwrap_or(r.property_name());
            println!(
                "  {} {} {} {} {}",
                style("\u{2192}").yellow(),
                style(name).cyan(),
                style(&r.current_version).dim(),
                style("\u{2192}").yellow(),
                style(r.latest_version().unwrap_or("?")).green()
            );
        }
    }
}
