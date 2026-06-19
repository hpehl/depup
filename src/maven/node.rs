//! Node.js version resolver for Maven POM tool-version properties.
//!
//! Resolves properties like `version.node` or `nodejs.version` against the
//! Node.js distribution index. When `--stable` is set, only LTS releases
//! are considered.

use std::future::Future;
use std::pin::Pin;

use anyhow::Result;
use serde::Deserialize;

use crate::constants::{self, NODEJS_DIST_URL};
use crate::maven::discovery::VersionProperty;
use crate::maven::tool::ToolVersionResolver;
use crate::model::{CheckResult, Dependency, DependencyKind, Ecosystem};
use crate::version;

/// Property name patterns that trigger Node.js version resolution.
const NODE_PATTERNS: &[&str] = &[
    "version.node",
    "version.nodejs",
    "node.version",
    "nodejs.version",
];

/// Resolves Node.js version properties against the Node.js distribution index.
pub struct NodeResolver {
    releases_only: bool,
}

/// A single release entry from the Node.js distribution index.
#[derive(Deserialize)]
struct NodeRelease {
    version: String,
    lts: LtsField,
}

/// The `lts` field in the Node.js index is either a codename string (e.g., "Jod")
/// or `false` for non-LTS releases.
#[derive(Deserialize)]
#[serde(untagged)]
enum LtsField {
    Name(#[allow(dead_code)] String),
    False(#[allow(dead_code)] bool),
}

impl LtsField {
    fn is_lts(&self) -> bool {
        matches!(self, Self::Name(_))
    }
}

fn tool_id(source: &str) -> Dependency {
    Dependency::new(
        Ecosystem::Maven,
        DependencyKind::Tool,
        "nodejs.org".to_string(),
        None,
        source.to_string(),
    )
}

impl NodeResolver {
    pub fn new(stable: bool) -> Self {
        Self {
            releases_only: stable,
        }
    }

    async fn fetch_and_check(
        &self,
        property: &VersionProperty,
        source: &str,
    ) -> Result<CheckResult> {
        let id = tool_id(source);
        let current = property.current_value.clone();

        let releases: Vec<NodeRelease> = constants::fetch_json(NODEJS_DIST_URL).await?;

        let versions: Vec<String> = releases
            .iter()
            .filter(|r| !self.releases_only || r.lts.is_lts())
            .map(|r| {
                r.version
                    .strip_prefix('v')
                    .unwrap_or(&r.version)
                    .to_string()
            })
            .collect();

        if versions.is_empty() {
            return Ok(CheckResult::error(
                id,
                current,
                "No Node.js versions found".to_string(),
            ));
        }

        let Some(latest) = version::find_latest(&versions) else {
            return Ok(CheckResult::error(
                id,
                current,
                "Could not determine latest Node.js version".to_string(),
            ));
        };

        let current_normalized = current.strip_prefix('v').unwrap_or(&current);
        let is_outdated = version::is_newer(current_normalized, &latest);
        Ok(CheckResult::checked(id, current, latest, is_outdated))
    }
}

impl ToolVersionResolver for NodeResolver {
    fn patterns(&self) -> &[&str] {
        NODE_PATTERNS
    }

    fn label(&self, _property: &VersionProperty) -> String {
        "nodejs.org".to_string()
    }

    fn resolve<'a>(
        &'a self,
        property: &'a VersionProperty,
        source: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<CheckResult>> + Send + 'a>> {
        Box::pin(self.fetch_and_check(property, source))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn patterns_match_node() {
        let resolver = NodeResolver::new(false);
        let patterns = resolver.patterns();
        assert!(patterns.contains(&"version.node"));
        assert!(patterns.contains(&"version.nodejs"));
        assert!(patterns.contains(&"node.version"));
        assert!(patterns.contains(&"nodejs.version"));
    }

    #[test]
    fn lts_field_parsing() {
        let lts: LtsField = serde_json::from_str(r#""Jod""#).unwrap();
        assert!(lts.is_lts());

        let not_lts: LtsField = serde_json::from_str("false").unwrap();
        assert!(!not_lts.is_lts());
    }

    #[test]
    fn lts_false_is_not_lts() {
        let field: LtsField = serde_json::from_str("false").unwrap();
        assert!(!field.is_lts());

        // Also verify that `true` (unusual but valid JSON) does not count as LTS
        // since LTS requires a codename string
        let field_true: LtsField = serde_json::from_str("true").unwrap();
        assert!(!field_true.is_lts());
    }

    #[test]
    fn tool_id_returns_correct_dependency() {
        let dep = tool_id("pom.xml");
        assert_eq!(dep.ecosystem, Ecosystem::Maven);
        assert_eq!(dep.kind, DependencyKind::Tool);
        assert_eq!(dep.artifact, "nodejs.org");
        assert!(dep.property.is_none());
        assert_eq!(dep.source, "pom.xml");
    }

    #[test]
    fn node_patterns_contains_all_expected() {
        assert_eq!(NODE_PATTERNS.len(), 4);
        assert!(NODE_PATTERNS.contains(&"version.node"));
        assert!(NODE_PATTERNS.contains(&"version.nodejs"));
        assert!(NODE_PATTERNS.contains(&"node.version"));
        assert!(NODE_PATTERNS.contains(&"nodejs.version"));
    }

    #[test]
    fn label_returns_nodejs_org() {
        let resolver = NodeResolver::new(false);
        let prop = VersionProperty {
            name: "version.node".to_string(),
            current_value: "20.0.0".to_string(),
            source: PathBuf::from("pom.xml"),
        };
        assert_eq!(resolver.label(&prop), "nodejs.org");
    }
}
