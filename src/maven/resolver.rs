//! Resolves Maven dependency versions against upstream registries.
//!
//! Two-phase design: `discover()` builds the task list synchronously,
//! `resolve()` runs all tasks concurrently with semaphore-based rate limiting.
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
use crate::maven::maven_central::MavenVersionResolver;
use crate::maven::tool::{ToolResolverRegistry, ToolVersionResolver};
use crate::model::{CheckResult, Dependency, DependencyKind, Ecosystem};

/// A single unit of work: either a Maven artifact or a tool version to resolve.
enum ResolveTask {
    Maven {
        mapping: ArtifactMapping,
        resolver: Arc<MavenVersionResolver>,
    },
    Tool {
        property: VersionProperty,
        resolver: Arc<dyn ToolVersionResolver>,
    },
}

impl ResolveTask {
    fn label(&self) -> String {
        match self {
            Self::Maven { mapping, .. } => {
                format!("{}:{}", mapping.group_id, mapping.artifact_id)
            }
            Self::Tool {
                property, resolver, ..
            } => resolver.label(property),
        }
    }

    fn error_id(&self, root: &Path) -> (Dependency, String) {
        match self {
            Self::Maven { mapping, .. } => {
                let source = mapping
                    .referenced_in
                    .strip_prefix(root)
                    .unwrap_or(&mapping.referenced_in)
                    .display()
                    .to_string();
                let artifact = format!("{}:{}", mapping.group_id, mapping.artifact_id);
                let property = if mapping.has_version_property {
                    Some(mapping.property.name.clone())
                } else {
                    None
                };
                let id = Dependency::new(
                    Ecosystem::Maven,
                    match mapping.kind {
                        crate::maven::pom::ArtifactKind::Dependency => DependencyKind::Dependency,
                        crate::maven::pom::ArtifactKind::Plugin => DependencyKind::Plugin,
                    },
                    artifact,
                    property,
                    source,
                );
                (id, mapping.property.current_value.clone())
            }
            Self::Tool { property, .. } => {
                let id = Dependency::new(
                    Ecosystem::Maven,
                    DependencyKind::Tool,
                    property.name.clone(),
                    None,
                    "pom.xml".into(),
                );
                (id, property.current_value.clone())
            }
        }
    }
}

/// Pre-built list of resolve tasks, ready for concurrent execution.
pub struct PreparedResolves {
    tasks: Vec<ResolveTask>,
}

impl PreparedResolves {
    pub fn count(&self) -> usize {
        self.tasks.len()
    }
}

/// Discovery phase: walks the Maven module tree, builds resolve tasks for all
/// artifacts and orphan tool-version properties. Runs synchronously.
pub fn discover(root: &Path, stable: bool) -> Result<PreparedResolves> {
    let discovery_result = discovery::discover(root)?;

    let maven_resolver = Arc::new(MavenVersionResolver::new(
        stable,
        discovery_result.repositories,
    ));
    let tool_registry = ToolResolverRegistry::new(stable);

    let mut tasks: Vec<ResolveTask> = discovery_result
        .mappings
        .into_iter()
        .map(|mapping| ResolveTask::Maven {
            mapping,
            resolver: Arc::clone(&maven_resolver),
        })
        .collect();

    for property in discovery_result.orphan_properties {
        if let Some(resolver) = tool_registry.find(&property.name) {
            tasks.push(ResolveTask::Tool { property, resolver });
        }
    }

    Ok(PreparedResolves { tasks })
}

/// Execution phase: runs all prepared resolve tasks concurrently with a semaphore.
/// Errors are captured as `VersionResult::error` rather than propagated.
pub async fn resolve(
    root: &Path,
    prepared: PreparedResolves,
    bar: &ProgressBar,
) -> Vec<CheckResult> {
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS));
    let mut join_set = JoinSet::new();
    let root = root.to_path_buf();

    for task in prepared.tasks {
        let semaphore = Arc::clone(&semaphore);
        let label = task.label();
        let bar = bar.clone();
        let root = root.clone();

        join_set.spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            bar.set_message(label);
            let result = match task {
                ResolveTask::Maven {
                    ref mapping,
                    ref resolver,
                } => {
                    let source = mapping
                        .referenced_in
                        .strip_prefix(&root)
                        .unwrap_or(&mapping.referenced_in)
                        .display()
                        .to_string();
                    resolver
                        .resolve(mapping, &source)
                        .await
                        .unwrap_or_else(|e| {
                            let (id, current) = task.error_id(&root);
                            CheckResult::error(id, current, e.to_string())
                        })
                }
                ResolveTask::Tool {
                    ref property,
                    ref resolver,
                } => resolver
                    .resolve(property, "pom.xml")
                    .await
                    .unwrap_or_else(|e| {
                        let (id, current) = task.error_id(&root);
                        CheckResult::error(id, current, e.to_string())
                    }),
            };
            bar.inc(1);
            result
        });
    }

    join_set.join_all().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/multi-module")
    }

    #[test]
    fn discover_multi_module_fixture_has_tasks() {
        let root = fixture_path();
        let prepared = discover(&root, false).unwrap();
        // The fixture has 2 properties (version.junit, version.compiler.plugin)
        // mapped to 2 artifacts => at least 2 tasks
        assert!(
            prepared.count() >= 2,
            "Expected at least 2 tasks, got {}",
            prepared.count()
        );
    }

    #[test]
    fn discover_labels_contain_maven_coordinates() {
        let root = fixture_path();
        let prepared = discover(&root, false).unwrap();
        let labels: Vec<String> = prepared.tasks.iter().map(|t| t.label()).collect();
        // Should contain the Maven artifact coordinates from the fixture
        assert!(
            labels.iter().any(|l| l.contains("junit-jupiter")),
            "Expected a label containing 'junit-jupiter', got: {:?}",
            labels
        );
        assert!(
            labels.iter().any(|l| l.contains("maven-compiler-plugin")),
            "Expected a label containing 'maven-compiler-plugin', got: {:?}",
            labels
        );
    }

    #[test]
    fn resolve_task_error_id_for_tool() {
        let property = VersionProperty {
            name: "version.node".to_string(),
            current_value: "20.0.0".to_string(),
        };
        let resolver = Arc::new(crate::maven::node::NodeResolver::new(false));
        let task = ResolveTask::Tool { property, resolver };
        let root = PathBuf::from("/tmp");
        let (id, current) = task.error_id(&root);
        assert_eq!(id.kind, DependencyKind::Tool);
        assert_eq!(id.artifact, "version.node");
        assert_eq!(current, "20.0.0");
    }
}
