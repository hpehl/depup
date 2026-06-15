use std::sync::Arc;

use anyhow::Result;
use clap::ArgMatches;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::Instant;

use crate::args;
use crate::constants::MAX_CONCURRENT_REQUESTS;
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

    if !json {
        progress::step("\u{1f50d}", "Discovering dependencies...");
    }

    let pnpm_projects = crate::pnpm::discovery::discover(&root);

    if !has_maven && pnpm_projects.is_empty() {
        if json {
            println!("[]");
        } else {
            println!("No supported project found.");
        }
        return Ok(());
    }

    if !json {
        progress::step("\u{2699}\u{fe0f}", "Checking...");
    }

    let mut join_set: JoinSet<Vec<CheckResult>> = JoinSet::new();

    if has_maven {
        let root = root.clone();
        join_set.spawn(async move {
            crate::maven::checker::check(&root, releases_only)
                .await
                .unwrap_or_default()
        });
    }

    if !pnpm_projects.is_empty() {
        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS));
        for project in pnpm_projects {
            let semaphore = Arc::clone(&semaphore);
            let project_path = project.path.clone();
            let project_name = project.name.clone();
            let root = root.clone();
            join_set.spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();
                crate::pnpm::check_project(&project_path, &root)
                    .await
                    .unwrap_or_else(|e| {
                        let source = project_path
                            .strip_prefix(&root)
                            .unwrap_or(&project_path)
                            .join("package.json")
                            .display()
                            .to_string();
                        vec![CheckResult::error(
                            Ecosystem::Pnpm,
                            CheckerKind::Pnpm,
                            project_name,
                            String::new(),
                            None,
                            e.to_string(),
                        )
                        .with_source(source)]
                    })
            });
        }
    }

    let mut all_results: Vec<CheckResult> = Vec::new();
    let results = join_set.join_all().await;
    for batch in results {
        all_results.extend(batch);
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
