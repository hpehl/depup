//! Node.js version checker for Maven POM tool-version properties.
//!
//! Checks properties like `version.node` or `nodejs.version` against the
//! Node.js distribution index. When `--stable` is set, only LTS releases
//! are considered.

use std::future::Future;
use std::pin::Pin;

use anyhow::Result;
use serde::Deserialize;

use crate::constants::{self, NODEJS_DIST_URL};
use crate::dependency::{Dependency, DependencyKind, Ecosystem, VersionResult};
use crate::error::DepupError;
use crate::maven::discovery::VersionProperty;
use crate::maven::tool::ToolVersionChecker;
use crate::version;

/// Property name patterns that trigger Node.js version checking.
const NODE_PATTERNS: &[&str] = &[
    "version.node",
    "version.nodejs",
    "node.version",
    "nodejs.version",
];

/// Checks Node.js version properties against the Node.js distribution index.
pub struct NodeChecker {
    client: reqwest::Client,
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
        DependencyKind::ToolVersion,
        "nodejs.org".to_string(),
        None,
        source.to_string(),
    )
}

impl NodeChecker {
    pub fn new(stable: bool) -> Self {
        Self {
            client: constants::http_client(),
            releases_only: stable,
        }
    }

    async fn fetch_and_check(
        &self,
        property: &VersionProperty,
        source: &str,
    ) -> Result<VersionResult> {
        let id = tool_id(source);
        let current = property.current_value.clone();

        let resp = self
            .client
            .get(NODEJS_DIST_URL)
            .send()
            .await
            .map_err(|e| DepupError::http_request_failed(NODEJS_DIST_URL, &e.to_string()))?;

        if !resp.status().is_success() {
            return Err(DepupError::http_request_failed(
                NODEJS_DIST_URL,
                &format!("HTTP {}", resp.status()),
            )
            .into());
        }

        let releases: Vec<NodeRelease> = resp
            .json()
            .await
            .map_err(|e| DepupError::http_request_failed(NODEJS_DIST_URL, &e.to_string()))?;

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
            return Ok(VersionResult::error(
                id,
                current,
                "No Node.js versions found".to_string(),
            ));
        }

        let Some(latest) = version::find_latest(&versions) else {
            return Ok(VersionResult::error(
                id,
                current,
                "Could not determine latest Node.js version".to_string(),
            ));
        };

        let current_normalized = current.strip_prefix('v').unwrap_or(&current);
        let is_outdated = version::is_newer(current_normalized, &latest);
        Ok(VersionResult::checked(id, current, latest, is_outdated))
    }
}

impl ToolVersionChecker for NodeChecker {
    fn patterns(&self) -> &[&str] {
        NODE_PATTERNS
    }

    fn label(&self, _property: &VersionProperty) -> String {
        "nodejs.org".to_string()
    }

    fn check<'a>(
        &'a self,
        property: &'a VersionProperty,
        source: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<VersionResult>> + Send + 'a>> {
        Box::pin(self.fetch_and_check(property, source))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_match_node() {
        let checker = NodeChecker::new(false);
        let patterns = checker.patterns();
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
}
