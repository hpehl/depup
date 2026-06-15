//! Orchestrates Maven ecosystem checks.
//!
//! Two-phase design: `discover()` builds the task list synchronously,
//! `check()` runs all tasks concurrently with semaphore-based rate limiting.
//! This split allows the caller to count tasks for the progress bar before
//! starting async work.

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use indicatif::ProgressBar;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::constants::MAX_CONCURRENT_REQUESTS;
use crate::maven::discovery::{self, ArtifactMapping, VersionProperty};
use crate::maven::pom::ArtifactKind;
use crate::maven::maven_central::MavenChecker;
use crate::maven::tool::{ToolCheckerRegistry, ToolVersionChecker};
use crate::registry::{CheckResult, CheckerKind, Ecosystem};

/// A single unit of work: either a Maven artifact check or a tool version check.
enum CheckTask {
    Maven {
        mapping: ArtifactMapping,
        checker: Arc<MavenChecker>,
    },
    Tool {
        property: VersionProperty,
        checker: Arc<dyn ToolVersionChecker>,
    },
}

impl CheckTask {
    fn kind(&self) -> CheckerKind {
        match self {
            Self::Maven { mapping, .. } => match mapping.kind {
                ArtifactKind::Dependency => CheckerKind::Dependency,
                ArtifactKind::Plugin => CheckerKind::Plugin,
            },
            Self::Tool { .. } => CheckerKind::ToolVersion,
        }
    }

    fn label(&self) -> String {
        match self {
            Self::Maven { mapping, .. } => {
                format!("{}:{}", mapping.group_id, mapping.artifact_id)
            }
            Self::Tool {
                property, checker, ..
            } => checker.label(property),
        }
    }

    fn source_label(&self, root: &Path) -> String {
        match self {
            Self::Maven { mapping, .. } => mapping
                .referenced_in
                .strip_prefix(root)
                .unwrap_or(&mapping.referenced_in)
                .display()
                .to_string(),
            Self::Tool { .. } => "pom.xml".to_string(),
        }
    }
}

/// Pre-built list of check tasks, ready for concurrent execution.
pub struct PreparedChecks {
    tasks: Vec<CheckTask>,
}

impl PreparedChecks {
    pub fn count(&self) -> usize {
        self.tasks.len()
    }
}

/// Discovery phase: walks the Maven module tree, builds check tasks for all
/// artifacts and orphan tool-version properties. Runs synchronously.
pub fn discover(root: &Path, stable: bool) -> Result<PreparedChecks> {
    let discovery_result = discovery::discover(root)?;

    let maven_checker = Arc::new(MavenChecker::new(
        stable,
        discovery_result.repositories,
    ));
    let tool_registry = ToolCheckerRegistry::new(stable);

    let mut tasks: Vec<CheckTask> = discovery_result
        .mappings
        .into_iter()
        .map(|mapping| CheckTask::Maven {
            mapping,
            checker: Arc::clone(&maven_checker),
        })
        .collect();

    for property in discovery_result.orphan_properties {
        if let Some(checker) = tool_registry.find(&property.name) {
            tasks.push(CheckTask::Tool { property, checker });
        }
    }

    Ok(PreparedChecks { tasks })
}

/// Execution phase: runs all prepared check tasks concurrently with a semaphore.
/// Errors are captured as `CheckResult::error` rather than propagated.
pub async fn check(root: &Path, prepared: PreparedChecks, bar: &ProgressBar) -> Vec<CheckResult> {
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS));
    let mut join_set = JoinSet::new();

    for task in prepared.tasks {
        let semaphore = Arc::clone(&semaphore);
        let kind = task.kind();
        let label = task.label();
        let source = task.source_label(root);
        let bar = bar.clone();

        join_set.spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            bar.set_message(label);
            let result = match task {
                CheckTask::Maven {
                    ref mapping,
                    ref checker,
                } => {
                    let has_prop = mapping.has_version_property;
                    checker
                        .check(mapping, &source)
                        .await
                        .unwrap_or_else(|e| {
                            CheckResult::error(
                                Ecosystem::Maven,
                                kind,
                                mapping.property.name.clone(),
                                mapping.property.current_value.clone(),
                                Some(format!("{}:{}", mapping.group_id, mapping.artifact_id)),
                                e.to_string(),
                                source.clone(),
                            )
                        })
                        .with_version_property(has_prop)
                }
                CheckTask::Tool {
                    ref property,
                    ref checker,
                } => checker.check(property, &source).await.unwrap_or_else(|e| {
                    CheckResult::error(
                        Ecosystem::Maven,
                        kind,
                        property.name.clone(),
                        property.current_value.clone(),
                        None,
                        e.to_string(),
                        source.clone(),
                    )
                }),
            };
            bar.inc(1);
            result
        });
    }

    join_set.join_all().await
}
