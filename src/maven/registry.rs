use anyhow::Result;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::time::Duration;

use crate::constants::{HTTP_TIMEOUT_SECS, MAVEN_CENTRAL_URL};
use crate::error::DepupError;
use crate::maven::discovery::ArtifactMapping;
use crate::maven::pom::{ArtifactKind, Repository, RepositoryKind};
use crate::registry::{CheckResult, CheckerKind, Ecosystem};
use crate::version::{self, Version};

pub struct MavenChecker {
    client: reqwest::Client,
    releases_only: bool,
    repositories: Vec<Repository>,
}

impl MavenChecker {
    pub fn new(releases_only: bool, repositories: Vec<Repository>) -> Self {
        let client = reqwest::Client::builder()
            .user_agent(format!("depup/{}", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .build()
            .expect("Failed to create HTTP client");
        Self {
            client,
            releases_only,
            repositories,
        }
    }

    fn repo_urls_for(&self, kind: ArtifactKind) -> Vec<&str> {
        self.repositories
            .iter()
            .filter(|r| match kind {
                ArtifactKind::Dependency => r.kind == RepositoryKind::Standard,
                ArtifactKind::Plugin => r.kind == RepositoryKind::Plugin,
            })
            .map(|r| r.url.as_str())
            .collect()
    }

    async fn fetch_from_repo(
        &self,
        base_url: &str,
        group_id: &str,
        artifact_id: &str,
    ) -> Result<Vec<String>> {
        fetch_versions(&self.client, base_url, group_id, artifact_id).await
    }
}

fn checker_kind(kind: ArtifactKind) -> CheckerKind {
    match kind {
        ArtifactKind::Dependency => CheckerKind::Dependency,
        ArtifactKind::Plugin => CheckerKind::Plugin,
    }
}

impl MavenChecker {
    pub async fn check(&self, mapping: &ArtifactMapping) -> Result<CheckResult> {
        let artifact = format!("{}:{}", mapping.group_id, mapping.artifact_id);
        let kind = checker_kind(mapping.kind);

        if self.releases_only
            && let Some(parsed) = Version::parse(&mapping.property.current_value)
            && parsed.is_pre_release()
        {
            return Ok(CheckResult {
                ecosystem: Ecosystem::Maven,
                property_name: mapping.property.name.clone(),
                current_version: mapping.property.current_value.clone(),
                latest_version: None,
                outdated: false,
                skipped: true,
                error: None,
                artifact: Some(artifact),
                kind,
            });
        }

        // Try Maven Central first
        let central_result = self
            .fetch_from_repo(MAVEN_CENTRAL_URL, &mapping.group_id, &mapping.artifact_id)
            .await;

        let all_versions = match central_result {
            Ok(versions) if !versions.is_empty() => versions,
            _ => {
                // Maven Central failed or empty — try custom repos in parallel
                let custom_urls = self.repo_urls_for(mapping.kind);
                if custom_urls.is_empty() {
                    return match central_result {
                        Err(e) => Ok(CheckResult {
                            ecosystem: Ecosystem::Maven,
                            property_name: mapping.property.name.clone(),
                            current_version: mapping.property.current_value.clone(),
                            latest_version: None,
                            outdated: false,
                            skipped: false,
                            error: Some(e.to_string()),
                            artifact: Some(artifact),
                            kind,
                        }),
                        Ok(_) => Ok(CheckResult {
                            ecosystem: Ecosystem::Maven,
                            property_name: mapping.property.name.clone(),
                            current_version: mapping.property.current_value.clone(),
                            latest_version: None,
                            outdated: false,
                            skipped: false,
                            error: Some(format!(
                                "No versions found for {}:{}",
                                mapping.group_id, mapping.artifact_id
                            )),
                            artifact: Some(artifact),
                            kind,
                        }),
                    };
                }

                let mut repo_tasks = tokio::task::JoinSet::new();
                for url in custom_urls {
                    let client = self.client.clone();
                    let group_id = mapping.group_id.clone();
                    let artifact_id = mapping.artifact_id.clone();
                    let url = url.to_string();
                    repo_tasks.spawn(async move {
                        fetch_versions(&client, &url, &group_id, &artifact_id).await
                    });
                }

                let repo_results = repo_tasks.join_all().await;
                let mut merged: Vec<String> = repo_results
                    .into_iter()
                    .filter_map(Result::ok)
                    .flatten()
                    .collect();

                if merged.is_empty() {
                    return Ok(CheckResult {
                        ecosystem: Ecosystem::Maven,
                        property_name: mapping.property.name.clone(),
                        current_version: mapping.property.current_value.clone(),
                        latest_version: None,
                        outdated: false,
                        skipped: false,
                        error: Some(format!(
                            "No versions found for {}:{}",
                            mapping.group_id, mapping.artifact_id
                        )),
                        artifact: Some(artifact),
                        kind,
                    });
                }

                merged.sort();
                merged.dedup();
                merged
            }
        };

        let filtered = filter_versions(&all_versions, self.releases_only);
        if filtered.is_empty() {
            return Ok(CheckResult {
                ecosystem: Ecosystem::Maven,
                property_name: mapping.property.name.clone(),
                current_version: mapping.property.current_value.clone(),
                latest_version: None,
                outdated: false,
                skipped: false,
                error: Some(format!(
                    "No release versions found for {}:{}",
                    mapping.group_id, mapping.artifact_id
                )),
                artifact: Some(artifact),
                kind,
            });
        }

        let latest = find_latest(&filtered);
        Ok(CheckResult {
            ecosystem: Ecosystem::Maven,
            property_name: mapping.property.name.clone(),
            current_version: mapping.property.current_value.clone(),
            latest_version: Some(latest.clone()),
            outdated: version::is_newer(&mapping.property.current_value, &latest),
            skipped: false,
            error: None,
            artifact: Some(artifact),
            kind,
        })
    }
}

async fn fetch_versions(
    client: &reqwest::Client,
    base_url: &str,
    group_id: &str,
    artifact_id: &str,
) -> Result<Vec<String>> {
    let group_path = group_id.replace('.', "/");
    let url = format!(
        "{}/{}/{}/maven-metadata.xml",
        base_url.trim_end_matches('/'),
        group_path,
        artifact_id
    );

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| DepupError::http_request_failed(&url, &e.to_string()))?;

    if !resp.status().is_success() {
        return Err(
            DepupError::http_request_failed(&url, &format!("HTTP {}", resp.status())).into(),
        );
    }

    let body = resp
        .text()
        .await
        .map_err(|e| DepupError::http_request_failed(&url, &e.to_string()))?;

    Ok(parse_metadata_versions(&body))
}

fn parse_metadata_versions(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    let mut versions = Vec::new();
    let mut path_stack: Vec<String> = Vec::new();
    let mut text_buf = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let local = name.split(':').next_back().unwrap_or(&name).to_string();
                path_stack.push(local);
                text_buf.clear();
            }
            Ok(Event::End(_)) => {
                let is_version_element = path_stack.len() >= 3
                    && path_stack.last().map(String::as_str) == Some("version")
                    && path_stack.iter().any(|s| s == "versions");

                if is_version_element {
                    let v = text_buf.trim().to_string();
                    if !v.is_empty() {
                        versions.push(v);
                    }
                }

                text_buf.clear();
                path_stack.pop();
            }
            Ok(Event::Text(e)) => {
                if let Ok(unescaped) = e.unescape() {
                    text_buf.push_str(&unescaped);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    versions
}

fn filter_versions(versions: &[String], releases_only: bool) -> Vec<String> {
    versions
        .iter()
        .filter_map(|v| {
            if v.to_lowercase().contains("snapshot") {
                return None;
            }
            if releases_only
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
        .map_or_else(|| versions[0].clone(), |v| v.raw.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_metadata_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<metadata>
  <groupId>org.example</groupId>
  <artifactId>my-lib</artifactId>
  <versioning>
    <latest>2.0.0</latest>
    <release>2.0.0</release>
    <versions>
      <version>1.0.0</version>
      <version>1.1.0</version>
      <version>2.0.0</version>
    </versions>
  </versioning>
</metadata>"#;

        let versions = parse_metadata_versions(xml);
        assert_eq!(versions, vec!["1.0.0", "1.1.0", "2.0.0"]);
    }

    #[test]
    fn parse_metadata_with_snapshots_and_qualifiers() {
        let xml = r#"<metadata>
  <versioning>
    <versions>
      <version>1.0.0</version>
      <version>1.1.0-SNAPSHOT</version>
      <version>2.0.0-alpha1</version>
      <version>2.0.0.Final</version>
    </versions>
  </versioning>
</metadata>"#;

        let versions = parse_metadata_versions(xml);
        assert_eq!(
            versions,
            vec!["1.0.0", "1.1.0-SNAPSHOT", "2.0.0-alpha1", "2.0.0.Final"]
        );
    }

    #[test]
    fn parse_empty_metadata() {
        let xml = r#"<metadata><versioning><versions></versions></versioning></metadata>"#;
        let versions = parse_metadata_versions(xml);
        assert!(versions.is_empty());
    }

    #[test]
    fn filters_snapshots_by_default() {
        let versions = vec![
            "1.0.0".to_string(),
            "2.0.0-SNAPSHOT".to_string(),
            "1.5.0".to_string(),
        ];
        let filtered = filter_versions(&versions, false);
        assert_eq!(filtered, vec!["1.0.0", "1.5.0"]);
    }

    #[test]
    fn includes_pre_releases_by_default() {
        let versions = vec![
            "1.0.0".to_string(),
            "2.0.0-alpha1".to_string(),
            "1.5.0".to_string(),
            "2.0.0-RC1".to_string(),
        ];
        let filtered = filter_versions(&versions, false);
        assert_eq!(
            filtered,
            vec!["1.0.0", "2.0.0-alpha1", "1.5.0", "2.0.0-RC1"]
        );
    }

    #[test]
    fn filters_pre_releases_when_releases_only() {
        let versions = vec![
            "1.0.0".to_string(),
            "2.0.0-alpha1".to_string(),
            "1.5.0".to_string(),
            "2.0.0-RC1".to_string(),
        ];
        let filtered = filter_versions(&versions, true);
        assert_eq!(filtered, vec!["1.0.0", "1.5.0"]);
    }

    #[test]
    fn snapshots_always_filtered_even_when_not_releases_only() {
        let versions = vec!["1.0.0".to_string(), "2.0.0-SNAPSHOT".to_string()];
        let filtered = filter_versions(&versions, false);
        assert_eq!(filtered, vec!["1.0.0"]);
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

    #[test]
    fn repo_urls_for_dependency() {
        let repos = vec![
            Repository {
                id: None,
                name: None,
                url: "https://dep-repo.example.com".into(),
                kind: RepositoryKind::Standard,
            },
            Repository {
                id: None,
                name: None,
                url: "https://plugin-repo.example.com".into(),
                kind: RepositoryKind::Plugin,
            },
        ];
        let checker = MavenChecker::new(false, repos);
        let urls = checker.repo_urls_for(ArtifactKind::Dependency);
        assert_eq!(urls, vec!["https://dep-repo.example.com"]);
    }

    #[test]
    fn repo_urls_for_plugin() {
        let repos = vec![
            Repository {
                id: None,
                name: None,
                url: "https://dep-repo.example.com".into(),
                kind: RepositoryKind::Standard,
            },
            Repository {
                id: None,
                name: None,
                url: "https://plugin-repo.example.com".into(),
                kind: RepositoryKind::Plugin,
            },
        ];
        let checker = MavenChecker::new(false, repos);
        let urls = checker.repo_urls_for(ArtifactKind::Plugin);
        assert_eq!(urls, vec!["https://plugin-repo.example.com"]);
    }
}
