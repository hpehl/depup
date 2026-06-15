//! Maven repository version resolver.
//!
//! Queries `maven-metadata.xml` to find all published versions of an artifact.
//! Tries Maven Central first; if not found, falls back to custom repositories
//! declared in the POM. Standard repositories are used for dependencies,
//! plugin repositories for plugins. Filters snapshots by default and optionally
//! filters pre-releases when `--stable` is set.

use anyhow::Result;
use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::constants::{self, MAVEN_CENTRAL_URL};
use crate::dependency::{Dependency, DependencyKind, Ecosystem, VersionResult};
use crate::error::DepupError;
use crate::maven::discovery::ArtifactMapping;
use crate::maven::pom::{ArtifactKind, Repository, RepositoryKind};
use crate::version::{self, Version};

/// Resolves Maven artifact versions against Maven Central and custom repositories.
pub struct MavenVersionResolver {
    client: reqwest::Client,
    releases_only: bool,
    repositories: Vec<Repository>,
}

impl MavenVersionResolver {
    pub fn new(releases_only: bool, repositories: Vec<Repository>) -> Self {
        Self {
            client: constants::http_client(),
            releases_only,
            repositories,
        }
    }

    /// Returns custom repository URLs matching the artifact kind
    /// (standard repos for dependencies, plugin repos for plugins).
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

fn dependency_kind(kind: ArtifactKind) -> DependencyKind {
    match kind {
        ArtifactKind::Dependency => DependencyKind::Dependency,
        ArtifactKind::Plugin => DependencyKind::Plugin,
    }
}

/// Builds a `Dependency` from an `ArtifactMapping`.
fn dependency_from_mapping(mapping: &ArtifactMapping, source: &str) -> Dependency {
    let artifact = format!("{}:{}", mapping.group_id, mapping.artifact_id);
    let property = if mapping.has_version_property {
        Some(mapping.property.name.clone())
    } else {
        None
    };
    Dependency::new(
        Ecosystem::Maven,
        dependency_kind(mapping.kind),
        artifact,
        property,
        source.to_string(),
    )
}

impl MavenVersionResolver {
    /// Resolves a single Maven artifact to find newer versions.
    /// Tries Maven Central first, then custom repos in parallel on miss.
    pub async fn resolve(&self, mapping: &ArtifactMapping, source: &str) -> Result<VersionResult> {
        let id = dependency_from_mapping(mapping, source);
        let artifact = format!("{}:{}", mapping.group_id, mapping.artifact_id);
        let current = mapping.property.current_value.clone();

        if self.releases_only
            && let Some(parsed) = Version::parse(&current)
            && parsed.is_pre_release()
        {
            return Ok(VersionResult::skipped(id, current));
        }

        let central_result = self
            .fetch_from_repo(MAVEN_CENTRAL_URL, &mapping.group_id, &mapping.artifact_id)
            .await;

        let all_versions = match central_result {
            Ok(versions) if !versions.is_empty() => versions,
            _ => {
                let custom_urls = self.repo_urls_for(mapping.kind);
                if custom_urls.is_empty() {
                    return match central_result {
                        Err(e) => Ok(VersionResult::error(id, current, e.to_string())),
                        Ok(_) => Ok(VersionResult::error(
                            id,
                            current,
                            format!("No versions found for {artifact}"),
                        )),
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
                    return Ok(VersionResult::error(
                        id,
                        current,
                        format!("No versions found for {artifact}"),
                    ));
                }

                merged.sort();
                merged.dedup();
                merged
            }
        };

        let filtered = filter_versions(&all_versions, self.releases_only);
        if filtered.is_empty() {
            return Ok(VersionResult::error(
                id,
                current,
                format!("No release versions found for {artifact}"),
            ));
        }

        let Some(latest) = version::find_latest(&filtered) else {
            return Ok(VersionResult::error(
                id,
                current,
                format!("Could not determine latest version for {artifact}"),
            ));
        };

        let is_outdated = version::is_newer(&current, &latest);
        Ok(VersionResult::checked(id, current, latest, is_outdated))
    }
}

/// Fetches all published versions from a Maven repository's `maven-metadata.xml`.
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

/// Parses `<version>` elements from a `maven-metadata.xml` response.
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

/// Filters out snapshots (always) and pre-releases (when `releases_only` is true).
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
        assert_eq!(version::find_latest(&versions), Some("2.3.1".to_string()));
    }

    #[test]
    fn find_latest_with_qualifiers() {
        let versions = vec![
            "3.0.0.Final".to_string(),
            "3.1.0.Final".to_string(),
            "2.5.0.Final".to_string(),
        ];
        assert_eq!(
            version::find_latest(&versions),
            Some("3.1.0.Final".to_string())
        );
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
        let resolver = MavenVersionResolver::new(false, repos);
        let urls = resolver.repo_urls_for(ArtifactKind::Dependency);
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
        let resolver = MavenVersionResolver::new(false, repos);
        let urls = resolver.repo_urls_for(ArtifactKind::Plugin);
        assert_eq!(urls, vec!["https://plugin-repo.example.com"]);
    }
}
