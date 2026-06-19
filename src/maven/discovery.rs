//! Maven module tree discovery and version property mapping.
//!
//! Walks the module tree starting from the root `pom.xml`, follows `<modules>`
//! declarations recursively, and maps each artifact's version — either any
//! `${...}` property reference or a plain inline version — back to the
//! resolved value from `<properties>` in the root or child POMs. Root properties
//! take precedence on conflict. Skips `${project.*}` built-ins. Also collects
//! repositories declared across all POMs.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::DepupError;
use crate::maven::pom::{self, ArtifactKind, Repository};

/// A named property with its resolved value (e.g., `version.junit` → `5.10.0`).
#[derive(Debug, Clone)]
pub struct VersionProperty {
    pub name: String,
    pub current_value: String,
    pub source: PathBuf,
}

/// Maps a Maven artifact to its version property and source POM location.
#[derive(Debug, Clone)]
pub struct ArtifactMapping {
    pub property: VersionProperty,
    pub group_id: String,
    pub artifact_id: String,
    pub kind: ArtifactKind,
    pub has_version_property: bool,
}

/// Result of the Maven module tree discovery phase.
pub struct DiscoveryResult {
    pub mappings: Vec<ArtifactMapping>,
    pub orphan_properties: Vec<VersionProperty>,
    pub repositories: Vec<Repository>,
}

/// Discovers all Maven artifacts and their version properties starting from the root POM.
///
/// Returns artifact mappings (property → artifact), orphan properties (properties
/// not referenced by any artifact, potential tool versions), and all declared repositories.
pub fn discover(root: &Path) -> Result<DiscoveryResult> {
    let root_pom_path = root.join("pom.xml");
    if !root_pom_path.exists() {
        return Err(DepupError::pom_not_found(&root.display().to_string()).into());
    }

    let root_project = pom::parse_pom(&root_pom_path)?;
    let mut properties = root_project.properties.clone();
    inject_project_properties(&root_project, &mut properties);

    let mut child_pom_files = Vec::new();
    let canonical_root = root.canonicalize().unwrap_or_else(|e| {
        eprintln!(
            "Warning: failed to canonicalize project root '{}' ({}), using as-is",
            root.display(),
            e
        );
        root.to_path_buf()
    });
    collect_module_poms(root, &root_project, &mut child_pom_files, &canonical_root)?;

    let mut mappings = Vec::new();
    let mut repositories = Vec::new();

    let mut property_sources: HashMap<String, PathBuf> = HashMap::new();
    for name in properties.keys() {
        property_sources.insert(name.clone(), root_pom_path.clone());
    }

    extract_mappings(
        &root_project,
        &root_pom_path,
        &properties,
        &property_sources,
        &mut mappings,
    );
    repositories.extend(root_project.repositories.clone());

    for pom_path in &child_pom_files {
        let project = pom::parse_pom(pom_path)
            .with_context(|| format!("Failed to parse {}", pom_path.display()))?;

        // Merge child properties into the global map (root wins on conflict)
        for (name, value) in &project.properties {
            if !properties.contains_key(name) {
                properties.insert(name.clone(), value.clone());
                property_sources.insert(name.clone(), pom_path.clone());
            }
        }

        let mut child_properties = properties.clone();
        inject_project_properties_with_fallback(&project, &root_project, &mut child_properties);
        extract_mappings(
            &project,
            pom_path,
            &child_properties,
            &property_sources,
            &mut mappings,
        );
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
        .map(|(name, value)| {
            let source = property_sources
                .get(name.as_str())
                .cloned()
                .unwrap_or_else(|| PathBuf::from("pom.xml"));
            VersionProperty {
                name: name.clone(),
                current_value: resolve_value(value, &properties),
                source,
            }
        })
        .collect();

    Ok(DiscoveryResult {
        mappings,
        orphan_properties,
        repositories,
    })
}

/// Injects Maven implicit properties (`project.groupId`, `project.artifactId`, etc.)
/// into the properties map so `${project.*}` references resolve correctly.
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

/// Like `inject_project_properties`, but for child modules: uses the child's values
/// when present, falling back to the parent's values for inherited coordinates.
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

/// Extracts artifact-to-property mappings from a parsed POM.
/// Handles both `${version.*}` property references and plain inline versions.
/// Skips artifacts referencing `${project.*}` properties.
fn extract_mappings(
    project: &pom::Project,
    pom_path: &Path,
    properties: &HashMap<String, String>,
    property_sources: &HashMap<String, PathBuf>,
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

        let (prop_name, current_value, has_version_property) =
            if let Some(prop_name) = extract_property_reference(version_str) {
                if prop_name.starts_with("project.") {
                    continue;
                }
                let Some(raw_value) = properties.get(&prop_name) else {
                    continue;
                };
                (prop_name, resolve_value(raw_value, properties), true)
            } else {
                let coords = format!(
                    "{}:{}",
                    resolve_value(group_id, properties),
                    resolve_value(artifact_id, properties)
                );
                (coords, version_str.trim().to_string(), false)
            };

        let group_id = resolve_value(group_id, properties);
        let artifact_id = resolve_value(artifact_id, properties);

        let source = if has_version_property {
            property_sources
                .get(&prop_name)
                .cloned()
                .unwrap_or_else(|| pom_path.to_path_buf())
        } else {
            pom_path.to_path_buf()
        };

        mappings.push(ArtifactMapping {
            property: VersionProperty {
                name: prop_name,
                current_value,
                source,
            },
            group_id,
            artifact_id,
            kind: *kind,
            has_version_property,
        });
    }
}

/// Recursively follows `<modules>` declarations to collect all child POM paths.
/// Validates that module paths don't escape the project root (path traversal protection).
fn collect_module_poms(
    parent_dir: &Path,
    project: &pom::Project,
    pom_files: &mut Vec<PathBuf>,
    project_root: &Path,
) -> Result<()> {
    for module_name in &project.modules {
        let module_dir = parent_dir.join(module_name);
        let module_pom = module_dir.join("pom.xml");
        if module_pom.exists() {
            match module_pom.canonicalize() {
                Ok(canonical) if canonical.starts_with(project_root) => {
                    pom_files.push(module_pom.clone());
                    let module_project = pom::parse_pom(&module_pom)?;
                    collect_module_poms(&module_dir, &module_project, pom_files, project_root)?;
                }
                Ok(_) => {
                    eprintln!("Warning: module '{module_name}' escapes project root, skipping");
                }
                Err(_) => {
                    eprintln!("Warning: cannot resolve module path '{module_name}', skipping");
                }
            }
        }
    }
    Ok(())
}

/// Extracts the property name from a `${property.name}` reference.
fn extract_property_reference(version: &str) -> Option<String> {
    version
        .trim()
        .strip_prefix("${")
        .and_then(|s| s.strip_suffix('}'))
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
}

const MAX_PROPERTY_RESOLUTION_DEPTH: usize = 10;

/// Resolves chained `${...}` property references up to a fixed depth.
fn resolve_value(value: &str, properties: &HashMap<String, String>) -> String {
    if !value.contains("${") {
        return value.to_string();
    }
    let mut current = value.to_string();
    for _ in 0..MAX_PROPERTY_RESOLUTION_DEPTH {
        match extract_property_reference(&current) {
            Some(prop_name) => match properties.get(&prop_name) {
                Some(resolved) => current.clone_from(resolved),
                None => return current,
            },
            None => return current,
        }
    }
    eprintln!(
        "Warning: property resolution depth limit ({}) reached for '{}'",
        MAX_PROPERTY_RESOLUTION_DEPTH, value
    );
    current
}

/// Removes duplicate artifact mappings by `groupId:artifactId`, keeping the first occurrence.
fn deduplicate(mappings: &mut Vec<ArtifactMapping>) {
    let mut seen = std::collections::HashSet::new();
    mappings.retain(|m| seen.insert(format!("{}:{}", m.group_id, m.artifact_id)));
}

/// Removes duplicate repositories by normalized URL (trailing slashes stripped).
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
                source: PathBuf::from("pom.xml"),
            },
            group_id: group.to_string(),
            artifact_id: artifact.to_string(),
            kind: pom::ArtifactKind::Dependency,
            has_version_property: !name.contains(':'),
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

        let mut property_sources = HashMap::new();
        property_sources.insert(
            "version.junit".to_string(),
            PathBuf::from("pom.xml"),
        );

        let mut mappings = Vec::new();
        extract_mappings(&project, Path::new("pom.xml"), &properties, &property_sources, &mut mappings);

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
    fn discover_child_pom_properties() {
        let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("child-properties");

        let result = discover(&fixture_dir).unwrap();

        // Should find both the root property (version.junit) and the child property (quarkus.platform.version)
        let names: Vec<&str> = result
            .mappings
            .iter()
            .map(|m| m.property.name.as_str())
            .collect();
        assert!(
            names.contains(&"quarkus.platform.version"),
            "Expected child property 'quarkus.platform.version' in mappings, got: {:?}",
            names
        );
        assert!(
            names.contains(&"version.junit"),
            "Expected root property 'version.junit' in mappings, got: {:?}",
            names
        );

        // The child property should resolve to the value defined in the child POM
        let quarkus = result
            .mappings
            .iter()
            .find(|m| m.property.name == "quarkus.platform.version")
            .unwrap();
        assert_eq!(quarkus.property.current_value, "3.36.2");
        assert_eq!(quarkus.group_id, "io.quarkus.platform");
        assert_eq!(quarkus.artifact_id, "quarkus-bom");

        // The child property's source should point to the child POM
        assert!(
            quarkus.property.source.ends_with("child/pom.xml"),
            "Expected child/pom.xml, got: {}",
            quarkus.property.source.display()
        );
    }

    #[test]
    fn root_property_wins_over_child_on_conflict() {
        let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("child-properties");

        let result = discover(&fixture_dir).unwrap();

        // version.junit is defined in root — it should keep the root value
        let junit = result
            .mappings
            .iter()
            .find(|m| m.property.name == "version.junit")
            .unwrap();
        assert_eq!(junit.property.current_value, "5.10.0");
    }

    #[test]
    fn discover_profile_module_properties() {
        let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("profile-modules");

        let result = discover(&fixture_dir).unwrap();

        let names: Vec<&str> = result
            .mappings
            .iter()
            .map(|m| m.property.name.as_str())
            .collect();
        assert!(
            names.contains(&"quarkus.platform.version"),
            "Expected 'quarkus.platform.version' from profile module, got: {:?}",
            names
        );
        assert!(
            names.contains(&"version.junit"),
            "Expected root 'version.junit', got: {:?}",
            names
        );

        let quarkus = result
            .mappings
            .iter()
            .find(|m| m.property.name == "quarkus.platform.version")
            .unwrap();
        assert_eq!(quarkus.property.current_value, "3.36.2");
        assert_eq!(quarkus.group_id, "io.quarkus.platform");
        assert_eq!(quarkus.artifact_id, "quarkus-bom");
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

    #[test]
    fn cross_pom_property_source_points_to_defining_pom() {
        let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("cross-pom-property");

        let result = discover(&fixture_dir).unwrap();

        let lib = result
            .mappings
            .iter()
            .find(|m| m.property.name == "version.lib")
            .expect("Expected mapping for version.lib");
        assert_eq!(lib.property.current_value, "1.0.0");
        assert_eq!(lib.group_id, "com.example");
        assert_eq!(lib.artifact_id, "some-lib");

        // Property is defined in root pom.xml, not child/pom.xml
        assert!(
            lib.property.source.ends_with("pom.xml"),
            "Expected root pom.xml, got: {}",
            lib.property.source.display()
        );
        assert!(
            !lib.property.source.ends_with("child/pom.xml"),
            "Source should NOT point to child/pom.xml, got: {}",
            lib.property.source.display()
        );
    }
}
