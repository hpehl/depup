//! Applies version updates to Maven POM files.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::maven::pom_writer::{self, InlineVersionUpdate};
use crate::registry::{CheckResult, UpdateResult};

/// Applies updates to POM files for all outdated Maven check results.
///
/// Updates both managed properties (those with `${...}` references) and
/// inline versions. A single POM may receive both types of updates.
///
/// # Arguments
/// * `root` - Root directory of the Maven project
/// * `outdated` - Pre-filtered outdated Maven check results
///
/// # Returns
/// A vector of `UpdateResult` entries — one per updated dependency.
pub fn apply_updates(root: &Path, outdated: &[CheckResult]) -> Result<Vec<UpdateResult>> {
    let mut update_results = Vec::new();

    // Group ALL updates (managed + inline) by POM path
    struct PomUpdates<'a> {
        properties: HashMap<String, &'a CheckResult>,
        inline: Vec<&'a CheckResult>,
    }

    let mut updates_by_pom: HashMap<String, PomUpdates> = HashMap::new();

    for result in outdated {
        let source = result.source().to_string();

        let entry = updates_by_pom.entry(source).or_insert_with(|| PomUpdates {
            properties: HashMap::new(),
            inline: Vec::new(),
        });

        if result.has_version_property() {
            entry
                .properties
                .insert(result.property_name().to_string(), result);
        } else {
            entry.inline.push(result);
        }
    }

    // Read, update, write each POM
    for (source, pom_updates) in updates_by_pom {
        let pom_path = root.join(&source);
        let xml = fs::read_to_string(&pom_path)
            .with_context(|| format!("Failed to read POM at {}", pom_path.display()))?;

        let mut updated_xml = xml;

        // Apply property updates
        if !pom_updates.properties.is_empty() {
            let property_map: HashMap<String, String> = pom_updates
                .properties
                .iter()
                .map(|(prop, r)| {
                    (
                        prop.clone(),
                        r.latest_version()
                            .expect("outdated result must have latest version")
                            .to_string(),
                    )
                })
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
                .filter_map(|r| {
                    let coords = r.property_name();
                    coords
                        .split_once(':')
                        .map(|(gid, aid)| InlineVersionUpdate {
                            group_id: gid.to_string(),
                            artifact_id: aid.to_string(),
                            new_version: r
                                .latest_version()
                                .expect("outdated result must have latest version")
                                .to_string(),
                        })
                })
                .collect();

            updated_xml = pom_writer::update_inline_versions(&updated_xml, &inline_updates)
                .with_context(|| {
                    format!("Failed to update inline versions in {}", pom_path.display())
                })?;
        }

        match fs::write(&pom_path, updated_xml) {
            Ok(()) => {
                // Record successful updates
                for check_result in pom_updates.properties.values() {
                    update_results.push(UpdateResult::updated(
                        check_result,
                        check_result
                            .latest_version()
                            .expect("outdated result must have latest version")
                            .to_string(),
                    ));
                }
                for check_result in &pom_updates.inline {
                    update_results.push(UpdateResult::updated(
                        check_result,
                        check_result
                            .latest_version()
                            .expect("outdated result must have latest version")
                            .to_string(),
                    ));
                }
            }
            Err(e) => {
                let message = format!("Failed to write POM: {e}");
                for check_result in pom_updates.properties.values() {
                    update_results.push(UpdateResult::error(check_result, message.clone()));
                }
                for check_result in &pom_updates.inline {
                    update_results.push(UpdateResult::error(check_result, message.clone()));
                }
            }
        }
    }

    // Sort by property name for stable output
    update_results.sort_by(|a, b| a.property_name.cmp(&b.property_name));

    Ok(update_results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{CheckId, CheckerKind, Ecosystem};
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

        let updated = apply_updates(temp.path(), &results).unwrap();

        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].property_name, "version.foo");
        assert_eq!(updated[0].old_version, "1.0.0");
        assert_eq!(updated[0].new_version, "1.1.0");
        assert!(!updated[0].is_error());

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

        let updated = apply_updates(temp.path(), &results).unwrap();

        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].property_name, "com.google.guava:guava");
        assert_eq!(updated[0].old_version, "30.0");
        assert_eq!(updated[0].new_version, "31.0");
        assert!(!updated[0].is_error());

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

        // Pass an empty slice since the caller pre-filters to outdated
        let updated = apply_updates(temp.path(), &[]).unwrap();

        assert!(updated.is_empty());

        // POM should be unchanged
        let unchanged_xml = fs::read_to_string(&pom_path).unwrap();
        assert_eq!(unchanged_xml, xml);
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

        let updated = apply_updates(temp.path(), &results).unwrap();

        assert_eq!(updated.len(), 2);

        // Find the property update
        let property_update = updated
            .iter()
            .find(|u| u.property_name == "version.junit")
            .unwrap();
        assert_eq!(property_update.old_version, "5.10.0");
        assert_eq!(property_update.new_version, "5.11.0");

        // Find the inline update
        let inline_update = updated
            .iter()
            .find(|u| u.property_name == "com.google.guava:guava")
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
