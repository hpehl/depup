use anyhow::Result;
use serde::Deserialize;
use std::time::Duration;

use crate::constants::{HTTP_TIMEOUT_SECS, NODEJS_DIST_URL};
use crate::discovery::VersionProperty;
use crate::error::MvnupError;
use crate::registry::{CheckResult, CheckerKind};
use crate::version::{self, Version};

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
        let client = reqwest::Client::builder()
            .user_agent(format!("mvnup/{}", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .build()
            .expect("Failed to create HTTP client");
        Self {
            client,
            releases_only,
        }
    }

    pub fn matches(property_name: &str) -> bool {
        NODE_PATTERNS.contains(&property_name)
    }

    pub async fn check(&self, property: &VersionProperty) -> Result<CheckResult> {
        let resp = self
            .client
            .get(NODEJS_DIST_URL)
            .send()
            .await
            .map_err(|e| MvnupError::http_request_failed(NODEJS_DIST_URL, &e.to_string()))?;

        if !resp.status().is_success() {
            return Err(MvnupError::http_request_failed(
                NODEJS_DIST_URL,
                &format!("HTTP {}", resp.status()),
            )
            .into());
        }

        let releases: Vec<NodeRelease> = resp
            .json()
            .await
            .map_err(|e| MvnupError::http_request_failed(NODEJS_DIST_URL, &e.to_string()))?;

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
            return Ok(CheckResult {
                property_name: property.name.clone(),
                current_version: property.current_value.clone(),
                latest_version: None,
                outdated: false,
                skipped: false,
                error: Some("No Node.js versions found".to_string()),
                artifact: Some("nodejs.org".to_string()),
                kind: CheckerKind::Node,
            });
        }

        let latest = find_latest(&versions);
        let current_normalized = property
            .current_value
            .strip_prefix('v')
            .unwrap_or(&property.current_value);

        Ok(CheckResult {
            property_name: property.name.clone(),
            current_version: property.current_value.clone(),
            latest_version: Some(latest.clone()),
            outdated: version::is_newer(current_normalized, &latest),
            skipped: false,
            error: None,
            artifact: Some("nodejs.org".to_string()),
            kind: CheckerKind::Node,
        })
    }
}

fn find_latest(versions: &[String]) -> String {
    let mut parsed: Vec<_> = versions.iter().filter_map(|v| Version::parse(v)).collect();
    parsed.sort();
    parsed
        .last()
        .map_or_else(|| versions[0].clone(), |v| v.raw.clone())
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
