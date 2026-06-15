//! Applies version updates to Maven POM files.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::maven::pom_writer::{self, InlineVersionUpdate};
use crate::registry::{CheckResult, Ecosystem};

/// Summary of a single property update.
#[derive(Debug, Clone)]
pub struct UpdateResult {
    pub property: String,
    pub old_version: String,
    pub new_version: String,
    pub pom_path: PathBuf,
}

/// Applies updates to POM files for all outdated Maven check results.
///
/// Updates both managed properties (those with `${...}` references) and
/// inline versions. A single POM may receive both types of updates.
///
/// # Arguments
/// * `root` - Root directory of the Maven project
/// * `results` - All check results from discovery and checking
///
/// # Returns
/// A tuple of (updated results, skipped messages).
/// Updated results contain property/artifact name, old/new versions, and POM path.
/// Skipped is returned empty for API compatibility but is no longer used.
pub fn apply_updates(
    root: &Path,
    results: &[CheckResult],
) -> Result<(Vec<UpdateResult>, Vec<String>)> {
    let mut updated_results = Vec::new();

    // Filter to only Maven + Outdated results
    let maven_outdated: Vec<&CheckResult> = results
        .iter()
        .filter(|r| r.ecosystem() == Ecosystem::Maven && r.is_outdated())
        .collect();

    // Group ALL updates (managed + inline) by POM path
    struct PomUpdates {
        properties: HashMap<String, (String, String)>, // property_name -> (old, new)
        inline: Vec<(String, String, String, String)>, // (groupId, artifactId, old, new)
    }

    let mut updates_by_pom: HashMap<PathBuf, PomUpdates> = HashMap::new();

    for result in maven_outdated {
        let pom_path = root.join(result.source());
        let new_version = result
            .latest_version()
            .expect("outdated result must have latest version")
            .to_string();

        let entry = updates_by_pom
            .entry(pom_path)
            .or_insert_with(|| PomUpdates {
                properties: HashMap::new(),
                inline: Vec::new(),
            });

        if result.has_version_property() {
            // Managed property update
            entry.properties.insert(
                result.property_name().to_string(),
                (result.current_version.clone(), new_version),
            );
        } else {
            // Inline version update
            // property_name is "groupId:artifactId" for inline versions
            let coords = result.property_name();
            if let Some((group_id, artifact_id)) = coords.split_once(':') {
                entry.inline.push((
                    group_id.to_string(),
                    artifact_id.to_string(),
                    result.current_version.clone(),
                    new_version,
                ));
            }
        }
    }

    // Read, update, write each POM
    for (pom_path, pom_updates) in updates_by_pom {
        let xml = fs::read_to_string(&pom_path)
            .with_context(|| format!("Failed to read POM at {}", pom_path.display()))?;

        let mut updated_xml = xml;

        // Apply property updates
        if !pom_updates.properties.is_empty() {
            let property_map: HashMap<String, String> = pom_updates
                .properties
                .iter()
                .map(|(prop, (_old, new))| (prop.clone(), new.clone()))
                .collect();

            updated_xml =
                pom_writer::update_properties(&updated_xml, &property_map).with_context(|| {
                    format!("Failed to update properties in {}", pom_path.display())
                })?;
        }

        // Apply inline version updates
        if !pom_updates.inline.is_empty() {
            let inline_updates: Vec<InlineVersionUpdate> = pom_updates
                .inline
                .iter()
                .map(|(gid, aid, _old, new)| InlineVersionUpdate {
                    group_id: gid.clone(),
                    artifact_id: aid.clone(),
                    new_version: new.clone(),
                })
                .collect();

            updated_xml = pom_writer::update_inline_versions(&updated_xml, &inline_updates)
                .with_context(|| {
                    format!("Failed to update inline versions in {}", pom_path.display())
                })?;
        }

        fs::write(&pom_path, updated_xml)
            .with_context(|| format!("Failed to write updated POM to {}", pom_path.display()))?;

        // Record what was updated
        for (property, (old_version, new_version)) in pom_updates.properties {
            updated_results.push(UpdateResult {
                property,
                old_version,
                new_version,
                pom_path: pom_path.clone(),
            });
        }

        for (group_id, artifact_id, old_version, new_version) in pom_updates.inline {
            updated_results.push(UpdateResult {
                property: format!("{}:{}", group_id, artifact_id),
                old_version,
                new_version,
                pom_path: pom_path.clone(),
            });
        }
    }

    // Sort updated results by property name
    updated_results.sort_by(|a, b| a.property.cmp(&b.property));

    Ok((updated_results, Vec::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{CheckId, CheckerKind};
    use tempfile::TempDir;

    fn outdated_result(
        property: &str,
        artifact: &str,
        current: &str,
        latest: &str,
        source: &str,
        has_version_property: bool,
    ) -> CheckResult {
        CheckResult::checked(
            CheckId::new(
                Ecosystem::Maven,
                CheckerKind::Dependency,
                property.to_string(),
                Some(artifact.to_string()),
                source.to_string(),
            )
            .with_version_property(has_version_property),
            current.to_string(),
            latest.to_string(),
            true,
        )
    }

    #[test]
    fn updates_managed_property_in_pom() {
        let temp = TempDir::new().unwrap();
        let pom_path = temp.path().join("pom.xml");

        let xml = r#"<project>
    <properties>
        <version.foo>1.0.0</version.foo>
        <version.bar>2.0.0</version.bar>
    </properties>
</project>"#;

        fs::write(&pom_path, xml).unwrap();

        let results = vec![outdated_result(
            "version.foo",
            "com.example:foo",
            "1.0.0",
            "1.1.0",
            "pom.xml",
            true,
        )];

        let (updated, skipped) = apply_updates(temp.path(), &results).unwrap();

        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].property, "version.foo");
        assert_eq!(updated[0].old_version, "1.0.0");
        assert_eq!(updated[0].new_version, "1.1.0");
        assert_eq!(updated[0].pom_path, pom_path);

        assert!(skipped.is_empty());

        // Verify the POM was actually updated
        let updated_xml = fs::read_to_string(&pom_path).unwrap();
        assert!(updated_xml.contains("<version.foo>1.1.0</version.foo>"));
        assert!(updated_xml.contains("<version.bar>2.0.0</version.bar>"));
    }

    #[test]
    fn updates_unmanaged_inline_versions() {
        let temp = TempDir::new().unwrap();
        let pom_path = temp.path().join("pom.xml");

        let xml = r#"<project>
    <dependencies>
        <dependency>
            <groupId>com.google.guava</groupId>
            <artifactId>guava</artifactId>
            <version>30.0</version>
        </dependency>
    </dependencies>
</project>"#;

        fs::write(&pom_path, xml).unwrap();

        let results = vec![outdated_result(
            "com.google.guava:guava",
            "com.google.guava:guava",
            "30.0",
            "31.0",
            "pom.xml",
            false,
        )];

        let (updated, skipped) = apply_updates(temp.path(), &results).unwrap();

        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].property, "com.google.guava:guava");
        assert_eq!(updated[0].old_version, "30.0");
        assert_eq!(updated[0].new_version, "31.0");
        assert_eq!(updated[0].pom_path, pom_path);

        assert!(skipped.is_empty());

        // Verify the POM was actually updated
        let updated_xml = fs::read_to_string(&pom_path).unwrap();
        assert!(updated_xml.contains("<version>31.0</version>"));
        assert!(updated_xml.contains("<groupId>com.google.guava</groupId>"));
        assert!(updated_xml.contains("<artifactId>guava</artifactId>"));
    }

    #[test]
    fn skips_non_outdated_results() {
        let temp = TempDir::new().unwrap();
        let pom_path = temp.path().join("pom.xml");

        let xml = r#"<project>
    <properties>
        <version.foo>1.0.0</version.foo>
    </properties>
</project>"#;

        fs::write(&pom_path, xml).unwrap();

        let results = vec![CheckResult::checked(
            CheckId::new(
                Ecosystem::Maven,
                CheckerKind::Dependency,
                "version.foo".to_string(),
                Some("com.example:foo".to_string()),
                "pom.xml".to_string(),
            )
            .with_version_property(true),
            "1.0.0".to_string(),
            "1.0.0".to_string(),
            false, // not outdated
        )];

        let (updated, skipped) = apply_updates(temp.path(), &results).unwrap();

        assert!(updated.is_empty());
        assert!(skipped.is_empty());

        // POM should be unchanged
        let unchanged_xml = fs::read_to_string(&pom_path).unwrap();
        assert_eq!(unchanged_xml, xml);
    }

    #[test]
    fn skips_non_maven_results() {
        let temp = TempDir::new().unwrap();

        let results = vec![CheckResult::checked(
            CheckId::new(
                Ecosystem::Npm,
                CheckerKind::NpmDep,
                "react".to_string(),
                None,
                "package.json".to_string(),
            ),
            "17.0.0".to_string(),
            "18.0.0".to_string(),
            true,
        )];

        let (updated, skipped) = apply_updates(temp.path(), &results).unwrap();

        assert!(updated.is_empty());
        assert!(skipped.is_empty());
    }

    #[test]
    fn updates_both_managed_and_inline_in_same_pom() {
        let temp = TempDir::new().unwrap();
        let pom_path = temp.path().join("pom.xml");

        let xml = r#"<project>
    <properties>
        <version.junit>5.10.0</version.junit>
    </properties>
    <dependencies>
        <dependency>
            <groupId>org.junit.jupiter</groupId>
            <artifactId>junit-jupiter</artifactId>
            <version>${version.junit}</version>
        </dependency>
        <dependency>
            <groupId>com.google.guava</groupId>
            <artifactId>guava</artifactId>
            <version>33.0.0-jre</version>
        </dependency>
    </dependencies>
</project>"#;

        fs::write(&pom_path, xml).unwrap();

        let results = vec![
            outdated_result(
                "version.junit",
                "org.junit.jupiter:junit-jupiter",
                "5.10.0",
                "5.11.0",
                "pom.xml",
                true,
            ),
            outdated_result(
                "com.google.guava:guava",
                "com.google.guava:guava",
                "33.0.0-jre",
                "33.4.0-jre",
                "pom.xml",
                false,
            ),
        ];

        let (updated, skipped) = apply_updates(temp.path(), &results).unwrap();

        assert_eq!(updated.len(), 2);
        assert!(skipped.is_empty());

        // Find the property update
        let property_update = updated
            .iter()
            .find(|u| u.property == "version.junit")
            .unwrap();
        assert_eq!(property_update.old_version, "5.10.0");
        assert_eq!(property_update.new_version, "5.11.0");

        // Find the inline update
        let inline_update = updated
            .iter()
            .find(|u| u.property == "com.google.guava:guava")
            .unwrap();
        assert_eq!(inline_update.old_version, "33.0.0-jre");
        assert_eq!(inline_update.new_version, "33.4.0-jre");

        // Verify the POM was updated for both
        let updated_xml = fs::read_to_string(&pom_path).unwrap();
        assert!(updated_xml.contains("<version.junit>5.11.0</version.junit>"));
        assert!(updated_xml.contains("<version>33.4.0-jre</version>"));
        assert!(updated_xml.contains("<version>${version.junit}</version>"));
    }
}
