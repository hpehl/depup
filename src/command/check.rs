use std::sync::Arc;

use anyhow::Result;
use clap::ArgMatches;
use indicatif::MultiProgress;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::Instant;

use crate::args;
use crate::constants::MAX_CONCURRENT_REQUESTS;
use crate::progress::{self, Progress};
use crate::registry::CheckResult;
use crate::registry::maven::MavenChecker;
use crate::{discovery, output};

pub async fn check(matches: &ArgMatches) -> Result<()> {
    let path = args::path_argument(matches);
    let json = args::is_json(matches);
    let outdated = args::is_outdated(matches);
    let verbose = args::is_verbose(matches);
    let include_pre_releases = args::include_pre_releases(matches);

    let instant = Instant::now();
    let root = path.canonicalize().unwrap_or(path.clone());

    if !json {
        progress::step("\u{1f50d}", "Discovering POM modules...");
    }
    let discovery_result = discovery::discover(&root)?;
    let mappings = discovery_result.mappings;

    if mappings.is_empty() {
        if json {
            println!("[]");
        } else {
            println!("No version properties with artifact mappings found.");
        }
        return Ok(());
    }

    if !json {
        progress::step(
            "\u{1f310}",
            &format!("Checking {} properties...", mappings.len()),
        );
    }

    let checker = Arc::new(MavenChecker::new(
        include_pre_releases,
        discovery_result.repositories,
    ));
    let mut results = check_all(checker, &mappings, json).await;
    results.sort_by(|a, b| a.property_name.cmp(&b.property_name));

    let filtered: Vec<CheckResult> = if outdated {
        results.into_iter().filter(|r| r.outdated).collect()
    } else {
        results
    };

    if json {
        output::print_json(&filtered);
    } else {
        println!();
        output::print_table(&filtered, verbose);
        progress::done(instant);
    }

    let has_outdated = filtered.iter().any(|r| r.outdated);
    if has_outdated {
        std::process::exit(1);
    }

    Ok(())
}

async fn check_all(
    checker: Arc<MavenChecker>,
    mappings: &[discovery::ArtifactMapping],
    json: bool,
) -> Vec<CheckResult> {
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS));
    let multi_progress = MultiProgress::new();
    let mut tasks = JoinSet::new();

    for mapping in mappings {
        let checker = Arc::clone(&checker);
        let semaphore = Arc::clone(&semaphore);
        let mapping = mapping.clone();
        let progress = if json {
            Progress::hidden(&mapping.property.name)
        } else {
            Progress::join(&multi_progress, &mapping.property.name)
        };

        tasks.spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            let result = match checker.check(&mapping).await {
                Ok(result) => result,
                Err(e) => CheckResult {
                    property_name: mapping.property.name.clone(),
                    current_version: mapping.property.current_value.clone(),
                    latest_version: None,
                    outdated: false,
                    error: Some(e.to_string()),
                    artifact: Some(format!("{}:{}", mapping.group_id, mapping.artifact_id)),
                },
            };
            if result.error.is_some() {
                progress.finish_error(result.error.as_deref().unwrap_or("unknown error"));
            } else {
                progress.finish_success();
            }
            result
        });
    }

    tasks.join_all().await
}
