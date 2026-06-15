use anyhow::Result;
use serde::Deserialize;

use crate::constants::{self, NODEJS_DIST_URL};
use crate::error::DepupError;
use crate::maven::discovery::VersionProperty;
use crate::registry::{CheckResult, CheckerKind, Ecosystem};
use crate::version;

const NODE_PATTERNS: &[&str] = &[
    "version.node",
    "version.nodejs",
    "node.version",
    "nodejs.version",
];

pub struct NodeChecker {
    client: reqwest::Client,
    releases_only: bool,
}

#[derive(Deserialize)]
struct NodeRelease {
    version: String,
    lts: LtsField,
}

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

impl NodeChecker {
    pub fn new(releases_only: bool) -> Self {
        Self {
            client: constants::http_client(),
            releases_only,
        }
    }

    pub fn matches(property_name: &str) -> bool {
        NODE_PATTERNS.contains(&property_name)
    }

    pub async fn check(&self, property: &VersionProperty, source: &str) -> Result<CheckResult> {
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

        let artifact = Some("nodejs.org".to_string());
        let prop_name = property.name.clone();
        let current = property.current_value.clone();
        let source = source.to_string();

        if versions.is_empty() {
            return Ok(CheckResult::error(
                Ecosystem::Maven,
                CheckerKind::Node,
                prop_name,
                current,
                artifact,
                "No Node.js versions found".to_string(),
                source,
            ));
        }

        let Some(latest) = version::find_latest(&versions) else {
            return Ok(CheckResult::error(
                Ecosystem::Maven,
                CheckerKind::Node,
                prop_name,
                current,
                artifact,
                "Could not determine latest Node.js version".to_string(),
                source,
            ));
        };

        let current_normalized = current.strip_prefix('v').unwrap_or(&current);
        let is_outdated = version::is_newer(current_normalized, &latest);
        Ok(CheckResult::checked(
            Ecosystem::Maven,
            CheckerKind::Node,
            prop_name,
            current,
            latest,
            is_outdated,
            artifact,
            source,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_node_patterns() {
        assert!(NodeChecker::matches("version.node"));
        assert!(NodeChecker::matches("version.nodejs"));
        assert!(NodeChecker::matches("node.version"));
        assert!(NodeChecker::matches("nodejs.version"));
    }

    #[test]
    fn does_not_match_unrelated() {
        assert!(!NodeChecker::matches("version.junit"));
        assert!(!NodeChecker::matches("node"));
        assert!(!NodeChecker::matches("version.node.something"));
    }

    #[test]
    fn lts_field_parsing() {
        let lts: LtsField = serde_json::from_str(r#""Jod""#).unwrap();
        assert!(lts.is_lts());

        let not_lts: LtsField = serde_json::from_str("false").unwrap();
        assert!(!not_lts.is_lts());
    }
}
