//! Applies version updates to Maven POM files.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::maven::pom_writer;
use crate::registry::{CheckResult, Ecosystem};

/// Summary of a single property update.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct UpdateResult {
    pub property: String,
    pub old_version: String,
    pub new_version: String,
    pub pom_path: PathBuf,
}

/// Applies updates to POM files for all outdated Maven check results.
///
/// Only updates managed properties (those with `${...}` references).
/// Plain inline versions are skipped and returned separately.
///
/// # Arguments
/// * `root` - Root directory of the Maven project
/// * `results` - All check results from discovery and checking
///
/// # Returns
/// A tuple of (updated results, skipped messages).
/// Updated results contain property name, old/new versions, and POM path.
/// Skipped messages describe inline versions that cannot be auto-updated.
#[allow(dead_code)]
pub fn apply_updates(
    root: &Path,
    results: &[CheckResult],
) -> Result<(Vec<UpdateResult>, Vec<String>)> {
    let mut updated_results = Vec::new();
    let mut skipped = Vec::new();

    // Filter to only Maven + Outdated results
    let maven_outdated: Vec<&CheckResult> = results
        .iter()
        .filter(|r| r.ecosystem() == Ecosystem::Maven && r.is_outdated())
        .collect();

    // Separate managed (has_version_property=true) from unmanaged (inline)
    let (managed, unmanaged): (Vec<&&CheckResult>, Vec<&&CheckResult>) = maven_outdated
        .iter()
        .partition(|r| r.has_version_property());

    // Group managed updates by source POM path
    let mut updates_by_pom: HashMap<PathBuf, HashMap<String, (String, String)>> = HashMap::new();

    for result in managed {
        let pom_path = root.join(result.source());
        let property = result.property_name().to_string();
        let old_version = result.current_version.clone();
        let new_version = result
            .latest_version()
            .expect("outdated result must have latest version")
            .to_string();

        updates_by_pom
            .entry(pom_path)
            .or_default()
            .insert(property, (old_version, new_version));
    }

    // Read, update, write each POM
    for (pom_path, property_updates) in updates_by_pom {
        let xml = fs::read_to_string(&pom_path)
            .with_context(|| format!("Failed to read POM at {}", pom_path.display()))?;

        // Build HashMap<property_name, new_version> for pom_writer
        let updates: HashMap<String, String> = property_updates
            .iter()
            .map(|(prop, (_old, new))| (prop.clone(), new.clone()))
            .collect();

        let updated_xml = pom_writer::update_properties(&xml, &updates)
            .with_context(|| format!("Failed to update properties in {}", pom_path.display()))?;

        fs::write(&pom_path, updated_xml)
            .with_context(|| format!("Failed to write updated POM to {}", pom_path.display()))?;

        // Record what was updated
        for (property, (old_version, new_version)) in property_updates {
            updated_results.push(UpdateResult {
                property,
                old_version,
                new_version,
                pom_path: pom_path.clone(),
            });
        }
    }

    // Add skipped messages for unmanaged inline versions
    for result in unmanaged {
        let artifact: String = result
            .artifact()
            .map(|a: &str| a.to_string())
            .unwrap_or_else(|| result.property_name().to_string());
        skipped.push(format!("{} (inline version, update manually)", artifact));
    }

    // Sort updated results by property name
    updated_results.sort_by(|a, b| a.property.cmp(&b.property));

    Ok((updated_results, skipped))
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
    fn skips_unmanaged_inline_versions() {
        let temp = TempDir::new().unwrap();

        let results = vec![outdated_result(
            "guava",
            "com.google.guava:guava",
            "30.0",
            "31.0",
            "pom.xml",
            false,
        )];

        let (updated, skipped) = apply_updates(temp.path(), &results).unwrap();

        assert!(updated.is_empty());
        assert_eq!(skipped.len(), 1);
        assert_eq!(
            skipped[0],
            "com.google.guava:guava (inline version, update manually)"
        );
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
}
