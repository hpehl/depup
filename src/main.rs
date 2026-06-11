mod discovery;
mod output;
mod pom;
mod registry;
mod version;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use tokio::sync::Semaphore;

use registry::maven::MavenCentralChecker;
use registry::{CheckResult, VersionChecker};

const MAX_CONCURRENT_REQUESTS: usize = 10;

#[derive(Parser)]
#[command(
    name = "mvnup",
    version,
    about = "Check Maven version properties against upstream registries"
)]
struct Cli {
    /// Path to the Maven project root (defaults to current directory)
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Output results as JSON
    #[arg(long)]
    json: bool,

    /// Only show outdated properties
    #[arg(long)]
    outdated: bool,

    /// Include pre-release versions (alpha, beta, RC, milestone)
    #[arg(long)]
    include_pre_releases: bool,

    /// Verbose output (show artifact coordinates)
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let root = cli.path.canonicalize().unwrap_or(cli.path.clone());
    let mappings = discovery::discover(&root)?;

    if mappings.is_empty() {
        if cli.json {
            println!("[]");
        } else {
            println!("No version properties with artifact mappings found.");
        }
        return Ok(());
    }

    let checker = Arc::new(MavenCentralChecker::new(cli.include_pre_releases));
    let mut results = check_all(checker, &mappings).await;
    results.sort_by(|a, b| a.property_name.cmp(&b.property_name));

    let filtered: Vec<CheckResult> = if cli.outdated {
        results.into_iter().filter(|r| r.outdated).collect()
    } else {
        results
    };

    if cli.json {
        output::print_json(&filtered);
    } else {
        output::print_table(&filtered, cli.verbose);
    }

    let has_outdated = filtered.iter().any(|r| r.outdated);
    if has_outdated {
        std::process::exit(1);
    }

    Ok(())
}

async fn check_all(
    checker: Arc<dyn VersionChecker>,
    mappings: &[discovery::ArtifactMapping],
) -> Vec<CheckResult> {
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS));
    let mut handles = Vec::with_capacity(mappings.len());

    for mapping in mappings {
        let checker = Arc::clone(&checker);
        let semaphore = Arc::clone(&semaphore);
        let mapping = mapping.clone();

        handles.push(tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            match checker.check(&mapping).await {
                Ok(result) => result,
                Err(e) => CheckResult {
                    property_name: mapping.property.name.clone(),
                    current_version: mapping.property.current_value.clone(),
                    latest_version: None,
                    outdated: false,
                    error: Some(e.to_string()),
                    artifact: Some(format!("{}:{}", mapping.group_id, mapping.artifact_id)),
                },
            }
        }));
    }

    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        if let Ok(result) = handle.await {
            results.push(result);
        }
    }
    results
}
