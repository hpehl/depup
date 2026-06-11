use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::pom::{self, ArtifactKind};

#[derive(Debug, Clone)]
pub struct VersionProperty {
    pub name: String,
    pub current_value: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ArtifactMapping {
    pub property: VersionProperty,
    pub group_id: String,
    pub artifact_id: String,
    pub kind: ArtifactKind,
    pub referenced_in: PathBuf,
}

pub fn discover(root: &Path) -> Result<Vec<ArtifactMapping>> {
    let root_pom_path = root.join("pom.xml");
    if !root_pom_path.exists() {
        anyhow::bail!("No pom.xml found in {}", root.display());
    }

    let root_project = pom::parse_pom(&root_pom_path)?;
    let properties = root_project.properties.clone();

    let mut child_pom_files = Vec::new();
    collect_module_poms(root, &root_project, &mut child_pom_files)?;

    let mut mappings = Vec::new();

    extract_mappings(
        &root_project,
        &root_pom_path,
        &properties,
        &mut mappings,
    );

    for pom_path in &child_pom_files {
        let project = pom::parse_pom(pom_path)
            .with_context(|| format!("Failed to parse {}", pom_path.display()))?;
        extract_mappings(&project, pom_path, &properties, &mut mappings);
    }

    deduplicate(&mut mappings);
    mappings.sort_by(|a, b| a.property.name.cmp(&b.property.name));
    Ok(mappings)
}

fn extract_mappings(
    project: &pom::Project,
    pom_path: &Path,
    properties: &HashMap<String, String>,
    mappings: &mut Vec<ArtifactMapping>,
) {
    for (artifact, kind) in &project.artifacts {
        let version_str = match &artifact.version {
            Some(v) => v,
            None => continue,
        };

        let prop_name = match extract_property_reference(version_str) {
            Some(name) => name,
            None => continue,
        };

        let group_id = match &artifact.group_id {
            Some(g) => resolve_value(g, properties),
            None => continue,
        };

        let artifact_id = match &artifact.artifact_id {
            Some(a) => resolve_value(a, properties),
            None => continue,
        };

        let current_value = match properties.get(&prop_name) {
            Some(v) => v.to_string(),
            None => continue,
        };

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
        .map(|s| s.to_string())
}

fn resolve_value(value: &str, properties: &HashMap<String, String>) -> String {
    if let Some(prop_name) = extract_property_reference(value) {
        properties
            .get(&prop_name)
            .cloned()
            .unwrap_or_else(|| value.to_string())
    } else {
        value.to_string()
    }
}

fn deduplicate(mappings: &mut Vec<ArtifactMapping>) {
    let mut seen = std::collections::HashSet::new();
    mappings.retain(|m| seen.insert(m.property.name.clone()));
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
        assert_eq!(extract_property_reference("${}"), Some("".to_string()));
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
        assert_eq!(
            resolve_value("${project.groupId}", &props),
            "org.example"
        );
    }

    #[test]
    fn resolve_missing_property_returns_raw() {
        let props = HashMap::new();
        assert_eq!(
            resolve_value("${missing.prop}", &props),
            "${missing.prop}"
        );
    }

    #[test]
    fn discover_fixture_project() {
        let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("multi-module");

        let mappings = discover(&fixture_dir).unwrap();
        assert_eq!(mappings.len(), 2);

        let names: Vec<&str> = mappings.iter().map(|m| m.property.name.as_str()).collect();
        assert!(names.contains(&"version.compiler.plugin"));
        assert!(names.contains(&"version.junit"));

        let junit = mappings.iter().find(|m| m.property.name == "version.junit").unwrap();
        assert_eq!(junit.group_id, "org.junit.jupiter");
        assert_eq!(junit.artifact_id, "junit-jupiter");
        assert_eq!(junit.property.current_value, "5.10.0");
    }

    #[test]
    fn discover_missing_pom_fails() {
        let result = discover(Path::new("/nonexistent/path"));
        assert!(result.is_err());
    }
}
