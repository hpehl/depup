use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use clap::ArgMatches;
use indicatif::MultiProgress;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::Instant;

use crate::args;
use crate::constants::MAX_CONCURRENT_REQUESTS;
use crate::output;
use crate::progress::{self, Progress, ProgressOutcome};
use crate::registry::{CheckResult, CheckerKind, Ecosystem};

pub async fn check(matches: &ArgMatches) -> Result<()> {
    let path = args::path_argument(matches);
    let json = args::is_json(matches);
    let outdated = args::is_outdated(matches);
    let releases_only = args::releases_only(matches);

    let instant = Instant::now();
    let root = path.canonicalize().unwrap_or_else(|_| path.clone());

    let has_maven = root.join("pom.xml").exists();
    let has_pnpm = has_pnpm_signals(&root);

    if !has_maven && !has_pnpm {
        if json {
            println!("[]");
        } else {
            println!("No supported project found.");
        }
        return Ok(());
    }

    let mut all_results: Vec<CheckResult> = Vec::new();

    if has_maven {
        let results = crate::maven::checker::check(&root, json, releases_only).await?;
        all_results.extend(results);
    }
    if has_pnpm {
        let results = pnpm_check(&root, json).await?;
        all_results.extend(results);
    }

    let filtered: Vec<CheckResult> = if outdated {
        all_results.into_iter().filter(|r| r.outdated).collect()
    } else {
        all_results
    };

    if json {
        output::print_json(&filtered);
    } else {
        println!();
        output::print_results(&filtered);
        progress::done(instant);
    }

    let has_outdated = filtered.iter().any(|r| r.outdated);
    if has_outdated {
        std::process::exit(1);
    }

    Ok(())
}

fn has_pnpm_signals(root: &Path) -> bool {
    if root.join("pnpm-lock.yaml").exists() {
        return true;
    }
    if root.join("package.json").exists()
        && let Ok(content) = std::fs::read_to_string(root.join("package.json"))
        && content.contains("\"pnpm@")
    {
        return true;
    }
    false
}

// ------------------------------------------------------------------
// pnpm check
// ------------------------------------------------------------------

async fn pnpm_check(root: &Path, json: bool) -> Result<Vec<CheckResult>> {
    if !json {
        progress::step("\u{1f50d}", "Discovering pnpm projects...");
    }
    let projects = crate::pnpm::discovery::discover(root);

    if projects.is_empty() {
        if !json {
            println!("No pnpm projects found.");
        }
        return Ok(Vec::new());
    }

    if !json {
        progress::step(
            "\u{2699}\u{fe0f}",
            &format!("Checking {} project(s)...", projects.len()),
        );
    }

    let results = pnpm_check_all(&projects, json).await;
    Ok(results)
}

async fn pnpm_check_all(
    projects: &[crate::pnpm::discovery::PnpmProject],
    json: bool,
) -> Vec<CheckResult> {
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS));
    let multi_progress = MultiProgress::new();
    let mut join_set = JoinSet::new();

    for project in projects {
        let semaphore = Arc::clone(&semaphore);
        let project_path = project.path.clone();
        let project_name = project.name.clone();

        let progress = if json {
            Progress::hidden(CheckerKind::Pnpm, &project_name, "", "")
        } else {
            Progress::join(
                &multi_progress,
                CheckerKind::Pnpm,
                &project_name,
                &project_path.display().to_string(),
                "",
            )
        };

        join_set.spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            let results = crate::pnpm::check_project(&project_path).await;
            match results {
                Ok(check_results) => {
                    let outdated = check_results.iter().filter(|r| r.outdated).count();
                    if outdated > 0 {
                        let count = check_results.len();
                        let label = format!("{outdated}/{count} outdated");
                        progress.finish(ProgressOutcome::Outdated { latest: &label });
                    } else {
                        progress.finish(ProgressOutcome::UpToDate);
                    }
                    check_results
                }
                Err(e) => {
                    let msg = e.to_string();
                    progress.finish(ProgressOutcome::Error { message: &msg });
                    vec![CheckResult::error(
                        Ecosystem::Pnpm,
                        CheckerKind::Pnpm,
                        project_name,
                        String::new(),
                        None,
                        msg,
                    )]
                }
            }
        });
    }

    join_set.join_all().await.into_iter().flatten().collect()
}
