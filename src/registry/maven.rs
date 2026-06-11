use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use std::time::Duration;

use crate::discovery::ArtifactMapping;
use crate::registry::{CheckResult, VersionChecker};
use crate::version::{self, Version};

pub struct MavenCentralChecker {
    client: reqwest::Client,
    include_pre_releases: bool,
}

impl MavenCentralChecker {
    pub fn new(include_pre_releases: bool) -> Self {
        let client = reqwest::Client::builder()
            .user_agent(format!("mvnup/{}", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
        Self {
            client,
            include_pre_releases,
        }
    }
}

#[async_trait]
impl VersionChecker for MavenCentralChecker {
    async fn check(&self, mapping: &ArtifactMapping) -> Result<CheckResult> {
        let result = self
            .fetch_latest(&mapping.group_id, &mapping.artifact_id)
            .await;

        let artifact = format!("{}:{}", mapping.group_id, mapping.artifact_id);

        match result {
            Ok(latest) => Ok(CheckResult {
                property_name: mapping.property.name.clone(),
                current_version: mapping.property.current_value.clone(),
                latest_version: Some(latest.clone()),
                outdated: version::is_newer(&mapping.property.current_value, &latest),
                error: None,
                artifact: Some(artifact),
            }),
            Err(e) => Ok(CheckResult {
                property_name: mapping.property.name.clone(),
                current_version: mapping.property.current_value.clone(),
                latest_version: None,
                outdated: false,
                error: Some(e.to_string()),
                artifact: Some(artifact),
            }),
        }
    }
}

impl MavenCentralChecker {
    async fn fetch_latest(
        &self,
        group_id: &str,
        artifact_id: &str,
    ) -> Result<String> {
        let query = format!("g:\"{}\" AND a:\"{}\"", group_id, artifact_id);

        let resp = self
            .client
            .get("https://search.maven.org/solrsearch/select")
            .query(&[
                ("q", query.as_str()),
                ("rows", "100"),
                ("wt", "json"),
                ("core", "gav"),
            ])
            .send()
            .await
            .context("Maven Central request failed")?;

        let status = resp.status();
        let body = resp.text().await.context("Failed to read response body")?;

        if !status.is_success() {
            anyhow::bail!("Maven Central returned HTTP {status}");
        }

        let response: SearchResponse = serde_json::from_str(&body)
            .with_context(|| {
                format!("Failed to parse response for {}:{}", group_id, artifact_id)
            })?;

        let versions = parse_versions(&response, self.include_pre_releases);

        if versions.is_empty() {
            anyhow::bail!("No versions found for {}:{}", group_id, artifact_id);
        }

        Ok(find_latest(&versions))
    }
}

fn parse_versions(response: &SearchResponse, include_pre_releases: bool) -> Vec<String> {
    response
        .response
        .docs
        .iter()
        .filter_map(|doc| {
            let v = &doc.v;
            if v.to_lowercase().contains("snapshot") {
                return None;
            }
            if !include_pre_releases
                && let Some(parsed) = Version::parse(v)
                && parsed.is_pre_release()
            {
                return None;
            }
            Some(v.clone())
        })
        .collect()
}

fn find_latest(versions: &[String]) -> String {
    let mut parsed: Vec<_> = versions.iter().filter_map(|v| Version::parse(v)).collect();
    parsed.sort();
    parsed
        .last()
        .map(|v| v.raw.clone())
        .unwrap_or_else(|| versions[0].clone())
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    response: SearchResponseBody,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchResponseBody {
    #[serde(default)]
    docs: Vec<SearchDoc>,
}

#[derive(Debug, Deserialize)]
struct SearchDoc {
    v: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_response(versions: &[&str]) -> SearchResponse {
        SearchResponse {
            response: SearchResponseBody {
                docs: versions
                    .iter()
                    .map(|v| SearchDoc { v: v.to_string() })
                    .collect(),
            },
        }
    }

    #[test]
    fn filters_snapshots() {
        let resp = make_response(&["1.0.0", "2.0.0-SNAPSHOT", "1.5.0"]);
        let versions = parse_versions(&resp, false);
        assert_eq!(versions, vec!["1.0.0", "1.5.0"]);
    }

    #[test]
    fn filters_pre_releases() {
        let resp = make_response(&["1.0.0", "2.0.0-alpha1", "1.5.0", "2.0.0-RC1"]);
        let versions = parse_versions(&resp, false);
        assert_eq!(versions, vec!["1.0.0", "1.5.0"]);
    }

    #[test]
    fn includes_pre_releases_when_flag_set() {
        let resp = make_response(&["1.0.0", "2.0.0-alpha1", "1.5.0"]);
        let versions = parse_versions(&resp, true);
        assert_eq!(versions, vec!["1.0.0", "2.0.0-alpha1", "1.5.0"]);
    }

    #[test]
    fn snapshots_always_filtered_even_with_pre_release_flag() {
        let resp = make_response(&["1.0.0", "2.0.0-SNAPSHOT"]);
        let versions = parse_versions(&resp, true);
        assert_eq!(versions, vec!["1.0.0"]);
    }

    #[test]
    fn find_latest_version() {
        let versions = vec![
            "1.0.0".to_string(),
            "2.3.1".to_string(),
            "2.1.0".to_string(),
        ];
        assert_eq!(find_latest(&versions), "2.3.1");
    }

    #[test]
    fn find_latest_with_qualifiers() {
        let versions = vec![
            "3.0.0.Final".to_string(),
            "3.1.0.Final".to_string(),
            "2.5.0.Final".to_string(),
        ];
        assert_eq!(find_latest(&versions), "3.1.0.Final");
    }
}
