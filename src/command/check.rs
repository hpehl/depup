use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use clap::ArgMatches;
use indicatif::ProgressBar;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::Instant;

use crate::args;
use crate::constants::MAX_CONCURRENT_REQUESTS;
use crate::npm::discovery::NpmProject;
use crate::output;
use crate::progress;
use crate::registry::{CheckResult, CheckerKind, Ecosystem};

pub async fn check(matches: &ArgMatches) -> Result<()> {
    let path = args::path_argument(matches);
    let json = args::is_json(matches);
    let outdated = args::is_outdated(matches);
    let releases_only = args::releases_only(matches);

    let instant = Instant::now();
    let root = path.canonicalize().unwrap_or_else(|_| path.clone());

    let has_maven = root.join("pom.xml").exists();

    let maven_prepared = if has_maven {
        Some(crate::maven::checker::discover(&root, releases_only)?)
    } else {
        None
    };
    let npm_projects = crate::npm::discovery::discover(&root);

    let maven_count = maven_prepared.as_ref().map_or(0, |p| p.count());
    let npm_count = npm_projects.len();
    let total = maven_count + npm_count;

    if total == 0 {
        if json {
            println!("[]");
        } else {
            println!("No supported project found.");
        }
        return Ok(());
    }

    let bar = if json {
        ProgressBar::hidden()
    } else {
        progress::bar(total as u64)
    };

    let mut join_set: JoinSet<Vec<CheckResult>> = JoinSet::new();

    if let Some(prepared) = maven_prepared {
        let root = root.clone();
        let bar = bar.clone();
        join_set.spawn(async move { crate::maven::checker::check(&root, prepared, &bar).await });
    }

    spawn_npm_checks(&mut join_set, npm_projects, &root, &bar);

    let all_results: Vec<CheckResult> = join_set.join_all().await.into_iter().flatten().collect();

    bar.finish_and_clear();

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

    if filtered.iter().any(|r| r.outdated) {
        std::process::exit(1);
    }

    Ok(())
}

fn spawn_npm_checks(
    join_set: &mut JoinSet<Vec<CheckResult>>,
    projects: Vec<NpmProject>,
    root: &Path,
    bar: &ProgressBar,
) {
    if projects.is_empty() {
        return;
    }

    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS));
    for project in projects {
        let semaphore = Arc::clone(&semaphore);
        let root = root.to_path_buf();
        let bar = bar.clone();
        join_set.spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            bar.set_message(format!("{} ({})", project.name, project.package_manager));
            let project_name = project.name.clone();
            let project_path = project.path.clone();
            let results = crate::npm::check_project(&project, &root)
                .await
                .unwrap_or_else(|e| {
                    let source = project_path
                        .strip_prefix(&root)
                        .unwrap_or(&project_path)
                        .join("package.json")
                        .display()
                        .to_string();
                    vec![CheckResult::error(
                        Ecosystem::Npm,
                        CheckerKind::NpmDep,
                        project_name,
                        String::new(),
                        None,
                        e.to_string(),
                        source,
                    )]
                });
            bar.inc(1);
            results
        });
    }
}
