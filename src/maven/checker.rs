use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use indicatif::MultiProgress;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::constants::MAX_CONCURRENT_REQUESTS;
use crate::maven::discovery::{self, ArtifactMapping, VersionProperty};
use crate::maven::node::NodeChecker;
use crate::maven::npm::NpmChecker;
use crate::maven::pom::ArtifactKind;
use crate::maven::registry::MavenChecker;
use crate::progress::{self, Progress};
use crate::registry::{CheckResult, CheckerKind, Ecosystem};

enum CheckTask {
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

impl CheckTask {
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

pub async fn check(root: &Path, json: bool, releases_only: bool) -> Result<Vec<CheckResult>> {
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

    let mut tasks: Vec<CheckTask> = discovery_result
        .mappings
        .into_iter()
        .map(|mapping| CheckTask::Maven {
            mapping,
            checker: Arc::clone(&maven_checker),
        })
        .collect();

    for property in discovery_result.orphan_properties {
        if NodeChecker::matches(&property.name) {
            tasks.push(CheckTask::Node {
                property,
                checker: Arc::clone(&node_checker),
            });
        } else if let Some(package) = NpmChecker::matches(&property.name) {
            tasks.push(CheckTask::Npm {
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

    let results_with_progress = check_all(tasks, json).await;
    let results: Vec<CheckResult> = results_with_progress.into_iter().map(|(r, _)| r).collect();
    Ok(results)
}

async fn check_all(tasks: Vec<CheckTask>, json: bool) -> Vec<(CheckResult, Progress)> {
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
                CheckTask::Maven {
                    ref mapping,
                    ref checker,
                } => checker.check(mapping).await.unwrap_or_else(|e| {
                    CheckResult::error(
                        Ecosystem::Maven,
                        kind,
                        mapping.property.name.clone(),
                        mapping.property.current_value.clone(),
                        Some(format!("{}:{}", mapping.group_id, mapping.artifact_id)),
                        e.to_string(),
                    )
                }),
                CheckTask::Node {
                    ref property,
                    ref checker,
                } => checker.check(property).await.unwrap_or_else(|e| {
                    CheckResult::error(
                        Ecosystem::Maven,
                        kind,
                        property.name.clone(),
                        property.current_value.clone(),
                        Some("nodejs.org".to_string()),
                        e.to_string(),
                    )
                }),
                CheckTask::Npm {
                    ref property,
                    package,
                    ref checker,
                } => checker.check(property, package).await.unwrap_or_else(|e| {
                    CheckResult::error(
                        Ecosystem::Maven,
                        kind,
                        property.name.clone(),
                        property.current_value.clone(),
                        Some(package.to_string()),
                        e.to_string(),
                    )
                }),
            };
            progress.finish(result.outcome());
            (result, progress)
        });
    }

    join_set.join_all().await
}
