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
use crate::maven::discovery;
use crate::maven::discovery::{ArtifactMapping, VersionProperty};
use crate::maven::node::NodeChecker;
use crate::maven::npm::NpmChecker;
use crate::maven::pom::ArtifactKind;
use crate::maven::registry::MavenChecker;
use crate::output;
use crate::progress::{self, Progress};
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
        let results = maven_check(&root, json, releases_only).await?;
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
        output::print_summary(&filtered);
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
// Maven check
// ------------------------------------------------------------------

enum MavenCheckTask {
    Maven {
        mapping: ArtifactMapping,
        checker: Arc<MavenChecker>,
    },
    Node {
        property: VersionProperty,
        checker: Arc<NodeChecker>,
    },
    Npm {
        property: VersionProperty,
        package: &'static str,
        checker: Arc<NpmChecker>,
    },
}

impl MavenCheckTask {
    fn kind(&self) -> CheckerKind {
        match self {
            Self::Maven { mapping, .. } => match mapping.kind {
                ArtifactKind::Dependency => CheckerKind::Dependency,
                ArtifactKind::Plugin => CheckerKind::Plugin,
            },
            Self::Node { .. } => CheckerKind::Node,
            Self::Npm { .. } => CheckerKind::Npm,
        }
    }

    fn property_name(&self) -> &str {
        match self {
            Self::Maven { mapping, .. } => &mapping.property.name,
            Self::Node { property, .. } | Self::Npm { property, .. } => &property.name,
        }
    }

    fn current_value(&self) -> &str {
        match self {
            Self::Maven { mapping, .. } => &mapping.property.current_value,
            Self::Node { property, .. } | Self::Npm { property, .. } => &property.current_value,
        }
    }

    fn artifact_label(&self) -> String {
        match self {
            Self::Maven { mapping, .. } => {
                format!("{}:{}", mapping.group_id, mapping.artifact_id)
            }
            Self::Node { .. } => "nodejs.org".to_string(),
            Self::Npm { package, .. } => (*package).to_string(),
        }
    }
}

async fn maven_check(root: &Path, json: bool, releases_only: bool) -> Result<Vec<CheckResult>> {
    if !json {
        progress::step("\u{1f50d}", "Discovering POM modules...");
    }
    let discovery_result = discovery::discover(root)?;

    let maven_checker = Arc::new(MavenChecker::new(
        releases_only,
        discovery_result.repositories,
    ));
    let node_checker = Arc::new(NodeChecker::new(releases_only));
    let npm_checker = Arc::new(NpmChecker::new(releases_only));

    let mut tasks: Vec<MavenCheckTask> = discovery_result
        .mappings
        .into_iter()
        .map(|mapping| MavenCheckTask::Maven {
            mapping,
            checker: Arc::clone(&maven_checker),
        })
        .collect();

    for property in discovery_result.orphan_properties {
        if NodeChecker::matches(&property.name) {
            tasks.push(MavenCheckTask::Node {
                property,
                checker: Arc::clone(&node_checker),
            });
        } else if let Some(package) = NpmChecker::matches(&property.name) {
            tasks.push(MavenCheckTask::Npm {
                property,
                package,
                checker: Arc::clone(&npm_checker),
            });
        }
    }

    if tasks.is_empty() {
        if !json {
            println!("No version properties found.");
        }
        return Ok(Vec::new());
    }

    if !json {
        progress::step(
            "\u{2699}\u{fe0f}",
            &format!("Checking {} properties...", tasks.len()),
        );
    }

    let results_with_progress = maven_check_all(tasks, json).await;
    let results: Vec<CheckResult> = results_with_progress.into_iter().map(|(r, _)| r).collect();
    Ok(results)
}

async fn maven_check_all(tasks: Vec<MavenCheckTask>, json: bool) -> Vec<(CheckResult, Progress)> {
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS));
    let multi_progress = MultiProgress::new();
    let mut join_set = JoinSet::new();

    for task in tasks {
        let semaphore = Arc::clone(&semaphore);
        let kind = task.kind();
        let property_name = task.property_name().to_string();
        let current_value = task.current_value().to_string();
        let artifact_label = task.artifact_label();

        let progress = if json {
            Progress::hidden(kind, &property_name, &artifact_label, &current_value)
        } else {
            Progress::join(
                &multi_progress,
                kind,
                &property_name,
                &artifact_label,
                &current_value,
            )
        };

        join_set.spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            let result = match task {
                MavenCheckTask::Maven {
                    ref mapping,
                    ref checker,
                } => checker
                    .check(mapping)
                    .await
                    .unwrap_or_else(|e| CheckResult {
                        ecosystem: Ecosystem::Maven,
                        property_name: mapping.property.name.clone(),
                        current_version: mapping.property.current_value.clone(),
                        latest_version: None,
                        outdated: false,
                        skipped: false,
                        error: Some(e.to_string()),
                        artifact: Some(format!("{}:{}", mapping.group_id, mapping.artifact_id)),
                        kind,
                    }),
                MavenCheckTask::Node {
                    ref property,
                    ref checker,
                } => checker
                    .check(property)
                    .await
                    .unwrap_or_else(|e| CheckResult {
                        ecosystem: Ecosystem::Maven,
                        property_name: property.name.clone(),
                        current_version: property.current_value.clone(),
                        latest_version: None,
                        outdated: false,
                        skipped: false,
                        error: Some(e.to_string()),
                        artifact: Some("nodejs.org".to_string()),
                        kind,
                    }),
                MavenCheckTask::Npm {
                    ref property,
                    package,
                    ref checker,
                } => checker
                    .check(property, package)
                    .await
                    .unwrap_or_else(|e| CheckResult {
                        ecosystem: Ecosystem::Maven,
                        property_name: property.name.clone(),
                        current_version: property.current_value.clone(),
                        latest_version: None,
                        outdated: false,
                        skipped: false,
                        error: Some(e.to_string()),
                        artifact: Some(package.to_string()),
                        kind,
                    }),
            };
            progress.finish_with_result(&result);
            (result, progress)
        });
    }

    join_set.join_all().await
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
                    let count = check_results.len();
                    let outdated = check_results.iter().filter(|r| r.outdated).count();
                    if outdated > 0 {
                        progress.finish_with_result(&CheckResult {
                            ecosystem: Ecosystem::Pnpm,
                            property_name: project_name,
                            current_version: String::new(),
                            latest_version: None,
                            outdated: true,
                            skipped: false,
                            error: None,
                            artifact: Some(format!("{outdated}/{count} outdated")),
                            kind: CheckerKind::Pnpm,
                        });
                    } else {
                        progress.finish_with_result(&CheckResult {
                            ecosystem: Ecosystem::Pnpm,
                            property_name: project_name,
                            current_version: String::new(),
                            latest_version: None,
                            outdated: false,
                            skipped: false,
                            error: None,
                            artifact: Some(format!("{count} packages")),
                            kind: CheckerKind::Pnpm,
                        });
                    }
                    check_results
                }
                Err(e) => {
                    progress.finish_with_result(&CheckResult {
                        ecosystem: Ecosystem::Pnpm,
                        property_name: project_name.clone(),
                        current_version: String::new(),
                        latest_version: None,
                        outdated: false,
                        skipped: false,
                        error: Some(e.to_string()),
                        artifact: None,
                        kind: CheckerKind::Pnpm,
                    });
                    vec![CheckResult {
                        ecosystem: Ecosystem::Pnpm,
                        property_name: project_name,
                        current_version: String::new(),
                        latest_version: None,
                        outdated: false,
                        skipped: false,
                        error: Some(e.to_string()),
                        artifact: None,
                        kind: CheckerKind::Pnpm,
                    }]
                }
            }
        });
    }

    join_set.join_all().await.into_iter().flatten().collect()
}
