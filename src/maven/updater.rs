//! Applies version updates to Maven POM files.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use indicatif::ProgressBar;

use crate::maven::pom_writer::{self, InlineVersionUpdate};
use crate::model::{CheckResult, CommandResult, UpdateResult};

struct PomUpdates<'a> {
    properties: HashMap<String, &'a CheckResult>,
    inline: Vec<&'a CheckResult>,
}

impl<'a> PomUpdates<'a> {
    fn all_results(&self) -> impl Iterator<Item = &&'a CheckResult> {
        self.properties.values().chain(self.inline.iter())
    }
}

/// Applies updates to POM files for all outdated Maven check results.
pub fn apply_updates(
    root: &Path,
    outdated: &[CheckResult],
    bar: &ProgressBar,
) -> Result<Vec<UpdateResult>> {
    let mut update_results = Vec::new();

    let mut updates_by_pom: HashMap<String, PomUpdates> = HashMap::new();

    for result in outdated {
        let source = result.source().to_string();

        let entry = updates_by_pom.entry(source).or_insert_with(|| PomUpdates {
            properties: HashMap::new(),
            inline: Vec::new(),
        });

        if result.has_property() {
            entry
                .properties
                .insert(result.property().unwrap().to_string(), result);
        } else {
            entry.inline.push(result);
        }
    }

    // Read, update, write each POM
    for (source, pom_updates) in &updates_by_pom {
        bar.set_message(source.clone());
        let pom_path = root.join(source);
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
                    r.artifact()
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
                for check_result in pom_updates.all_results() {
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
                for check_result in pom_updates.all_results() {
                    update_results.push(UpdateResult::error(check_result, message.clone()));
                }
            }
        }
        bar.inc(1);
    }

    // Sort by artifact for stable output
    update_results.sort_by(|a, b| a.dep.artifact.cmp(&b.dep.artifact));

    Ok(update_results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Dependency, DependencyKind, Ecosystem};
    use indicatif::ProgressBar;
    use tempfile::TempDir;

    fn outdated_result(
        artifact: &str,
        property: Option<&str>,
        current: &str,
        latest: &str,
        source: &str,
    ) -> CheckResult {
        CheckResult::checked(
            Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                artifact.to_string(),
                property.map(ToString::to_string),
                source.to_string(),
            ),
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
            "com.example:foo",
            Some("version.foo"),
            "1.0.0",
            "1.1.0",
            "pom.xml",
        )];

        let updated = apply_updates(temp.path(), &results, &ProgressBar::hidden()).unwrap();

        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].dep.property, Some("version.foo".to_string()));
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
            None,
            "30.0",
            "31.0",
            "pom.xml",
        )];

        let updated = apply_updates(temp.path(), &results, &ProgressBar::hidden()).unwrap();

        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].dep.artifact, "com.google.guava:guava");
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
        let updated = apply_updates(temp.path(), &[], &ProgressBar::hidden()).unwrap();

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
                "org.junit.jupiter:junit-jupiter",
                Some("version.junit"),
                "5.10.0",
                "5.11.0",
                "pom.xml",
            ),
            outdated_result(
                "com.google.guava:guava",
                None,
                "33.0.0-jre",
                "33.4.0-jre",
                "pom.xml",
            ),
        ];

        let updated = apply_updates(temp.path(), &results, &ProgressBar::hidden()).unwrap();

        assert_eq!(updated.len(), 2);

        // Find the property update
        let property_update = updated
            .iter()
            .find(|u| u.dep.property == Some("version.junit".to_string()))
            .unwrap();
        assert_eq!(property_update.old_version, "5.10.0");
        assert_eq!(property_update.new_version, "5.11.0");

        // Find the inline update
        let inline_update = updated
            .iter()
            .find(|u| u.dep.artifact == "com.google.guava:guava")
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
