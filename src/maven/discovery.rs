use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::DepupError;
use crate::maven::pom::{self, ArtifactKind, Repository};

#[derive(Debug, Clone)]
pub struct VersionProperty {
    pub name: String,
    pub current_value: String,
}

#[derive(Debug, Clone)]
pub struct ArtifactMapping {
    pub property: VersionProperty,
    pub group_id: String,
    pub artifact_id: String,
    pub kind: ArtifactKind,
    #[allow(dead_code)]
    pub referenced_in: PathBuf,
}

pub struct DiscoveryResult {
    pub mappings: Vec<ArtifactMapping>,
    pub orphan_properties: Vec<VersionProperty>,
    pub repositories: Vec<Repository>,
}

pub fn discover(root: &Path) -> Result<DiscoveryResult> {
    let root_pom_path = root.join("pom.xml");
    if !root_pom_path.exists() {
        return Err(DepupError::pom_not_found(&root.display().to_string()).into());
    }

    let root_project = pom::parse_pom(&root_pom_path)?;
    let mut properties = root_project.properties.clone();
    inject_project_properties(&root_project, &mut properties);

    let mut child_pom_files = Vec::new();
    collect_module_poms(root, &root_project, &mut child_pom_files)?;

    let mut mappings = Vec::new();
    let mut repositories = Vec::new();

    extract_mappings(&root_project, &root_pom_path, &properties, &mut mappings);
    repositories.extend(root_project.repositories.clone());

    for pom_path in &child_pom_files {
        let project = pom::parse_pom(pom_path)
            .with_context(|| format!("Failed to parse {}", pom_path.display()))?;
        let mut child_properties = properties.clone();
        inject_project_properties_with_fallback(&project, &root_project, &mut child_properties);
        extract_mappings(&project, pom_path, &child_properties, &mut mappings);
        repositories.extend(project.repositories);
    }

    deduplicate(&mut mappings);
    mappings.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then(a.property.name.cmp(&b.property.name))
    });
    deduplicate_repos(&mut repositories);

    let matched_names: std::collections::HashSet<&str> =
        mappings.iter().map(|m| m.property.name.as_str()).collect();
    let orphan_properties: Vec<VersionProperty> = properties
        .iter()
        .filter(|(name, _)| !matched_names.contains(name.as_str()))
        .filter(|(name, _)| !name.starts_with("project."))
        .map(|(name, value)| VersionProperty {
            name: name.clone(),
            current_value: resolve_value(value, &properties),
        })
        .collect();

    Ok(DiscoveryResult {
        mappings,
        orphan_properties,
        repositories,
    })
}

fn inject_project_properties(project: &pom::Project, properties: &mut HashMap<String, String>) {
    if let Some(gid) = &project.group_id {
        properties.insert("project.groupId".to_string(), gid.clone());
    }
    if let Some(aid) = &project.artifact_id {
        properties.insert("project.artifactId".to_string(), aid.clone());
    }
    if let Some(ver) = &project.version {
        properties.insert("project.version".to_string(), ver.clone());
    }
    properties.insert(
        "project.packaging".to_string(),
        project
            .packaging
            .clone()
            .unwrap_or_else(|| "jar".to_string()),
    );
}

fn inject_project_properties_with_fallback(
    child: &pom::Project,
    parent: &pom::Project,
    properties: &mut HashMap<String, String>,
) {
    if let Some(gid) = child.group_id.as_ref().or(parent.group_id.as_ref()) {
        properties.insert("project.groupId".to_string(), gid.clone());
    }
    if let Some(aid) = &child.artifact_id {
        properties.insert("project.artifactId".to_string(), aid.clone());
    }
    if let Some(ver) = child.version.as_ref().or(parent.version.as_ref()) {
        properties.insert("project.version".to_string(), ver.clone());
    }
    properties.insert(
        "project.packaging".to_string(),
        child
            .packaging
            .clone()
            .or_else(|| parent.packaging.clone())
            .unwrap_or_else(|| "jar".to_string()),
    );
}

fn extract_mappings(
    project: &pom::Project,
    pom_path: &Path,
    properties: &HashMap<String, String>,
    mappings: &mut Vec<ArtifactMapping>,
) {
    for (artifact, kind) in &project.artifacts {
        let Some(version_str) = &artifact.version else {
            continue;
        };
        let Some(group_id) = &artifact.group_id else {
            continue;
        };
        let Some(artifact_id) = &artifact.artifact_id else {
            continue;
        };

        let (prop_name, current_value) =
            if let Some(prop_name) = extract_property_reference(version_str) {
                if prop_name.starts_with("project.") {
                    continue;
                }
                let Some(raw_value) = properties.get(&prop_name) else {
                    continue;
                };
                (prop_name, resolve_value(raw_value, properties))
            } else {
                let coords = format!(
                    "{}:{}",
                    resolve_value(group_id, properties),
                    resolve_value(artifact_id, properties)
                );
                (coords, version_str.trim().to_string())
            };

        let group_id = resolve_value(group_id, properties);
        let artifact_id = resolve_value(artifact_id, properties);

        mappings.push(ArtifactMapping {
            property: VersionProperty {
                name: prop_name,
                current_value,
            },
            group_id,
            artifact_id,
            kind: *kind,
            referenced_in: pom_path.to_path_buf(),
        });
    }
}

fn collect_module_poms(
    parent_dir: &Path,
    project: &pom::Project,
    pom_files: &mut Vec<PathBuf>,
) -> Result<()> {
    for module_name in &project.modules {
        let module_pom = parent_dir.join(module_name).join("pom.xml");
        if module_pom.exists() {
            pom_files.push(module_pom.clone());
            let module_project = pom::parse_pom(&module_pom)?;
            collect_module_poms(&parent_dir.join(module_name), &module_project, pom_files)?;
        }
    }
    Ok(())
}

fn extract_property_reference(version: &str) -> Option<String> {
    version
        .trim()
        .strip_prefix("${")
        .and_then(|s| s.strip_suffix('}'))
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
}

fn resolve_value(value: &str, properties: &HashMap<String, String>) -> String {
    let mut current = value.to_string();
    for _ in 0..10 {
        match extract_property_reference(&current) {
            Some(prop_name) => match properties.get(&prop_name) {
                Some(resolved) => current = resolved.clone(),
                None => return current,
            },
            None => return current,
        }
    }
    current
}

fn deduplicate(mappings: &mut Vec<ArtifactMapping>) {
    let mut seen = std::collections::HashSet::new();
    mappings.retain(|m| seen.insert(format!("{}:{}", m.group_id, m.artifact_id)));
}

fn deduplicate_repos(repos: &mut Vec<Repository>) {
    let mut seen = std::collections::HashSet::new();
    repos.retain(|r| {
        let normalized = r.url.trim_end_matches('/').to_string();
        seen.insert(normalized)
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_property_ref() {
        assert_eq!(
            extract_property_reference("${version.wildfly}"),
            Some("version.wildfly".to_string())
        );
        assert_eq!(extract_property_reference("1.0.0"), None);
        assert_eq!(extract_property_reference("${incomplete"), None);
        assert_eq!(extract_property_reference(""), None);
        assert_eq!(extract_property_reference("${}"), None);
    }

    #[test]
    fn extract_property_ref_with_whitespace() {
        assert_eq!(
            extract_property_reference("  ${version.junit}  "),
            Some("version.junit".to_string())
        );
    }

    #[test]
    fn resolve_plain_value() {
        let props = HashMap::new();
        assert_eq!(resolve_value("org.example", &props), "org.example");
    }

    #[test]
    fn resolve_property_value() {
        let mut props = HashMap::new();
        props.insert("project.groupId".to_string(), "org.example".to_string());
        assert_eq!(resolve_value("${project.groupId}", &props), "org.example");
    }

    #[test]
    fn resolve_chained_property_value() {
        let mut props = HashMap::new();
        props.insert("project.groupId".to_string(), "org.wildfly".to_string());
        props.insert(
            "ee.maven.groupId".to_string(),
            "${project.groupId}".to_string(),
        );
        assert_eq!(resolve_value("${ee.maven.groupId}", &props), "org.wildfly");
    }

    #[test]
    fn resolve_missing_property_returns_raw() {
        let props = HashMap::new();
        assert_eq!(resolve_value("${missing.prop}", &props), "${missing.prop}");
    }

    #[test]
    fn discover_fixture_project() {
        let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("multi-module");

        let result = discover(&fixture_dir).unwrap();
        assert_eq!(result.mappings.len(), 2);

        let names: Vec<&str> = result
            .mappings
            .iter()
            .map(|m| m.property.name.as_str())
            .collect();
        assert!(names.contains(&"version.compiler.plugin"));
        assert!(names.contains(&"version.junit"));

        let junit = result
            .mappings
            .iter()
            .find(|m| m.property.name == "version.junit")
            .unwrap();
        assert_eq!(junit.group_id, "org.junit.jupiter");
        assert_eq!(junit.artifact_id, "junit-jupiter");
        assert_eq!(junit.property.current_value, "5.10.0");
    }

    #[test]
    fn discover_missing_pom_fails() {
        let result = discover(Path::new("/nonexistent/path"));
        assert!(result.is_err());
    }

    #[test]
    fn deduplicate_repos_by_url() {
        use crate::maven::pom::{Repository, RepositoryKind};

        let mut repos = vec![
            Repository {
                id: Some("r1".into()),
                name: None,
                url: "https://repo.example.com/maven2/".into(),
                kind: RepositoryKind::Standard,
            },
            Repository {
                id: Some("r2".into()),
                name: None,
                url: "https://repo.example.com/maven2".into(),
                kind: RepositoryKind::Plugin,
            },
            Repository {
                id: Some("r3".into()),
                name: None,
                url: "https://other.example.com/repo".into(),
                kind: RepositoryKind::Standard,
            },
        ];

        deduplicate_repos(&mut repos);
        assert_eq!(repos.len(), 2);
        assert_eq!(repos[0].id.as_deref(), Some("r1"));
        assert_eq!(repos[1].id.as_deref(), Some("r3"));
    }

    #[test]
    fn inject_project_properties_sets_all_fields() {
        let project = pom::Project {
            group_id: Some("org.wildfly".into()),
            artifact_id: Some("wildfly-parent".into()),
            version: Some("35.0.0.Final".into()),
            packaging: Some("pom".into()),
            ..Default::default()
        };
        let mut props = HashMap::new();
        inject_project_properties(&project, &mut props);

        assert_eq!(props.get("project.groupId").unwrap(), "org.wildfly");
        assert_eq!(props.get("project.artifactId").unwrap(), "wildfly-parent");
        assert_eq!(props.get("project.version").unwrap(), "35.0.0.Final");
        assert_eq!(props.get("project.packaging").unwrap(), "pom");
    }

    #[test]
    fn inject_project_properties_defaults_packaging_to_jar() {
        let project = pom::Project {
            group_id: Some("org.example".into()),
            artifact_id: Some("my-lib".into()),
            version: Some("1.0.0".into()),
            ..Default::default()
        };
        let mut props = HashMap::new();
        inject_project_properties(&project, &mut props);

        assert_eq!(props.get("project.packaging").unwrap(), "jar");
    }

    #[test]
    fn inject_child_properties_uses_child_values() {
        let parent = pom::Project {
            group_id: Some("org.parent".into()),
            artifact_id: Some("parent".into()),
            version: Some("1.0.0".into()),
            ..Default::default()
        };
        let child = pom::Project {
            group_id: Some("org.child".into()),
            artifact_id: Some("child-mod".into()),
            version: Some("2.0.0".into()),
            packaging: Some("war".into()),
            ..Default::default()
        };
        let mut props = HashMap::new();
        inject_project_properties_with_fallback(&child, &parent, &mut props);

        assert_eq!(props.get("project.groupId").unwrap(), "org.child");
        assert_eq!(props.get("project.artifactId").unwrap(), "child-mod");
        assert_eq!(props.get("project.version").unwrap(), "2.0.0");
        assert_eq!(props.get("project.packaging").unwrap(), "war");
    }

    #[test]
    fn inject_child_properties_falls_back_to_parent() {
        let parent = pom::Project {
            group_id: Some("org.parent".into()),
            artifact_id: Some("parent".into()),
            version: Some("1.0.0".into()),
            packaging: Some("pom".into()),
            ..Default::default()
        };
        let child = pom::Project {
            artifact_id: Some("child-mod".into()),
            ..Default::default()
        };
        let mut props = HashMap::new();
        inject_project_properties_with_fallback(&child, &parent, &mut props);

        assert_eq!(props.get("project.groupId").unwrap(), "org.parent");
        assert_eq!(props.get("project.artifactId").unwrap(), "child-mod");
        assert_eq!(props.get("project.version").unwrap(), "1.0.0");
        assert_eq!(props.get("project.packaging").unwrap(), "pom");
    }

    #[test]
    fn deduplicate_by_coordinates() {
        let mapping = |name: &str, group: &str, artifact: &str| ArtifactMapping {
            property: VersionProperty {
                name: name.to_string(),
                current_value: "1.0.0".to_string(),
            },
            group_id: group.to_string(),
            artifact_id: artifact.to_string(),
            kind: pom::ArtifactKind::Dependency,
            referenced_in: PathBuf::from("pom.xml"),
        };

        let mut mappings = vec![
            mapping("version.guava", "com.google.guava", "guava"),
            mapping("com.google.guava:guava", "com.google.guava", "guava"),
            mapping("version.junit", "org.junit.jupiter", "junit-jupiter"),
        ];

        deduplicate(&mut mappings);
        assert_eq!(mappings.len(), 2);
        assert_eq!(mappings[0].property.name, "version.guava");
        assert_eq!(mappings[1].property.name, "version.junit");
    }

    #[test]
    fn extract_mappings_includes_plain_versions() {
        let project = pom::Project {
            artifacts: vec![
                (
                    pom::Artifact {
                        group_id: Some("com.google.guava".to_string()),
                        artifact_id: Some("guava".to_string()),
                        version: Some("33.0.0-jre".to_string()),
                    },
                    pom::ArtifactKind::Dependency,
                ),
                (
                    pom::Artifact {
                        group_id: Some("org.junit.jupiter".to_string()),
                        artifact_id: Some("junit-jupiter".to_string()),
                        version: Some("${version.junit}".to_string()),
                    },
                    pom::ArtifactKind::Dependency,
                ),
                (
                    pom::Artifact {
                        group_id: Some("org.example".to_string()),
                        artifact_id: Some("no-version".to_string()),
                        version: None,
                    },
                    pom::ArtifactKind::Dependency,
                ),
            ],
            ..Default::default()
        };

        let mut properties = HashMap::new();
        properties.insert("version.junit".to_string(), "5.10.0".to_string());

        let mut mappings = Vec::new();
        extract_mappings(
            &project,
            Path::new("pom.xml"),
            &properties,
            &mut mappings,
        );

        assert_eq!(mappings.len(), 2);

        let guava = mappings
            .iter()
            .find(|m| m.artifact_id == "guava")
            .expect("guava mapping should exist");
        assert_eq!(guava.property.name, "com.google.guava:guava");
        assert_eq!(guava.property.current_value, "33.0.0-jre");
        assert_eq!(guava.group_id, "com.google.guava");

        let junit = mappings
            .iter()
            .find(|m| m.artifact_id == "junit-jupiter")
            .expect("junit mapping should exist");
        assert_eq!(junit.property.name, "version.junit");
        assert_eq!(junit.property.current_value, "5.10.0");
    }

    #[test]
    fn discover_plain_versions_fixture() {
        let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("plain-versions");

        let result = discover(&fixture_dir).unwrap();
        assert_eq!(result.mappings.len(), 2);

        let names: Vec<&str> = result
            .mappings
            .iter()
            .map(|m| m.property.name.as_str())
            .collect();
        assert!(names.contains(&"version.junit"));
        assert!(names.contains(&"com.google.guava:guava"));

        let guava = result
            .mappings
            .iter()
            .find(|m| m.artifact_id == "guava")
            .unwrap();
        assert_eq!(guava.property.name, "com.google.guava:guava");
        assert_eq!(guava.property.current_value, "33.0.0-jre");
        assert_eq!(guava.group_id, "com.google.guava");
        assert_eq!(guava.artifact_id, "guava");
    }
}
