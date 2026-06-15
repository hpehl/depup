# Update Subcommand Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the `depup update` subcommand that updates outdated dependency versions in-place — rewriting Maven POM properties while preserving XML formatting, and delegating to native PM commands for npm.

**Architecture:** The update flow reuses the existing check pipeline (discovery → check → comparison) to identify what's outdated, then applies updates. For Maven, a new `pom_writer` module does surgical text-level replacements of `<properties>` values in the raw XML, avoiding DOM round-trips that destroy formatting. For npm, each `PackageManagerChecker` gains an `update_packages()` method that shells out to the PM's native update command. The `update` command reports what it changed.

**Tech Stack:** Rust, quick-xml (read-only, for locating byte offsets), tokio, existing `PackageManagerChecker` trait, `regex` crate for property location.

---

### Task 1: Maven POM Property Writer — Core Module

**Files:**
- Create: `src/maven/pom_writer.rs`
- Modify: `src/maven/mod.rs` (add `pub mod pom_writer;`)
- Test: `src/maven/pom_writer.rs` (inline `#[cfg(test)]` module)

This is the key module. It takes raw POM XML as a `&str`, a map of property names to new values, and returns a new `String` with only those property values replaced. All whitespace, comments, indentation, and XML structure are preserved because we work at the byte/string level, not through a DOM.

**Strategy:** Scan the XML for `<properties>` blocks, then within each block find `<propName>oldValue</propName>` and replace `oldValue` with the new value. We use quick-xml's event reader to locate byte positions, then do string splicing on the original text.

- [ ] **Step 1: Write the failing tests**

```rust
// src/maven/pom_writer.rs

//! Surgical POM XML property updater.
//!
//! Replaces property values inside `<properties>` blocks without altering
//! formatting, comments, or whitespace. Works at the byte level using
//! quick-xml only to locate element boundaries.

use std::collections::HashMap;

use anyhow::Result;

/// Replaces property values in raw POM XML, preserving all formatting.
///
/// Only touches `<propName>value</propName>` elements inside top-level
/// `<properties>` blocks. Returns a new string with the replacements applied.
pub fn update_properties(xml: &str, updates: &HashMap<String, String>) -> Result<String> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updates_single_property() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <properties>
        <version.junit>5.10.0</version.junit>
        <version.lombok>1.18.30</version.lombok>
    </properties>
</project>"#;

        let mut updates = HashMap::new();
        updates.insert("version.junit".to_string(), "5.12.0".to_string());

        let result = update_properties(xml, &updates).unwrap();
        assert!(result.contains("<version.junit>5.12.0</version.junit>"));
        assert!(result.contains("<version.lombok>1.18.30</version.lombok>"));
    }

    #[test]
    fn updates_multiple_properties() {
        let xml = r#"<project>
    <properties>
        <version.junit>5.10.0</version.junit>
        <version.wildfly>35.0.0.Final</version.wildfly>
    </properties>
</project>"#;

        let mut updates = HashMap::new();
        updates.insert("version.junit".to_string(), "5.12.0".to_string());
        updates.insert("version.wildfly".to_string(), "36.0.0.Final".to_string());

        let result = update_properties(xml, &updates).unwrap();
        assert!(result.contains("<version.junit>5.12.0</version.junit>"));
        assert!(result.contains("<version.wildfly>36.0.0.Final</version.wildfly>"));
    }

    #[test]
    fn preserves_comments() {
        let xml = r#"<project>
    <properties>
        <!-- JUnit testing framework -->
        <version.junit>5.10.0</version.junit>
    </properties>
</project>"#;

        let mut updates = HashMap::new();
        updates.insert("version.junit".to_string(), "5.12.0".to_string());

        let result = update_properties(xml, &updates).unwrap();
        assert!(result.contains("<!-- JUnit testing framework -->"));
        assert!(result.contains("<version.junit>5.12.0</version.junit>"));
    }

    #[test]
    fn preserves_indentation_and_whitespace() {
        let xml = "  <project>\n\t\t<properties>\n\t\t\t<version.junit>5.10.0</version.junit>\n\t\t</properties>\n  </project>";

        let mut updates = HashMap::new();
        updates.insert("version.junit".to_string(), "5.12.0".to_string());

        let result = update_properties(xml, &updates).unwrap();
        assert!(result.contains("\t\t\t<version.junit>5.12.0</version.junit>"));
    }

    #[test]
    fn ignores_properties_not_in_updates() {
        let xml = r#"<project>
    <properties>
        <version.junit>5.10.0</version.junit>
        <maven.compiler.source>17</maven.compiler.source>
    </properties>
</project>"#;

        let mut updates = HashMap::new();
        updates.insert("version.junit".to_string(), "5.12.0".to_string());

        let result = update_properties(xml, &updates).unwrap();
        assert!(result.contains("<maven.compiler.source>17</maven.compiler.source>"));
    }

    #[test]
    fn handles_xml_namespaces() {
        let xml = r#"<project xmlns="http://maven.apache.org/POM/4.0.0">
    <properties>
        <version.junit>5.10.0</version.junit>
    </properties>
</project>"#;

        let mut updates = HashMap::new();
        updates.insert("version.junit".to_string(), "5.12.0".to_string());

        let result = update_properties(xml, &updates).unwrap();
        assert!(result.contains("<version.junit>5.12.0</version.junit>"));
        assert!(result.contains("xmlns="));
    }

    #[test]
    fn no_updates_returns_unchanged() {
        let xml = r#"<project>
    <properties>
        <version.junit>5.10.0</version.junit>
    </properties>
</project>"#;

        let updates = HashMap::new();

        let result = update_properties(xml, &updates).unwrap();
        assert_eq!(result, xml);
    }

    #[test]
    fn preserves_trailing_newline() {
        let xml = "<project>\n    <properties>\n        <version.junit>5.10.0</version.junit>\n    </properties>\n</project>\n";

        let mut updates = HashMap::new();
        updates.insert("version.junit".to_string(), "5.12.0".to_string());

        let result = update_properties(xml, &updates).unwrap();
        assert!(result.ends_with('\n'));
    }

    #[test]
    fn handles_property_with_whitespace_value() {
        let xml = r#"<project>
    <properties>
        <version.junit>  5.10.0  </version.junit>
    </properties>
</project>"#;

        let mut updates = HashMap::new();
        updates.insert("version.junit".to_string(), "5.12.0".to_string());

        let result = update_properties(xml, &updates).unwrap();
        assert!(result.contains("<version.junit>5.12.0</version.junit>"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test maven::pom_writer::tests -v`
Expected: FAIL — all tests hit `todo!()`

- [ ] **Step 3: Implement `update_properties`**

The approach: use a regex-based scanner to find property elements inside `<properties>` blocks. For each match where the property name is in our updates map, splice in the new value. We work on byte offsets of the original string, collecting replacements, then build the result by copying unchanged segments interleaved with new values.

```rust
use anyhow::{Context, Result};
use std::collections::HashMap;

/// Replaces property values in raw POM XML, preserving all formatting.
///
/// Only touches `<propName>value</propName>` elements inside top-level
/// `<properties>` blocks. Returns a new string with the replacements applied.
pub fn update_properties(xml: &str, updates: &HashMap<String, String>) -> Result<String> {
    if updates.is_empty() {
        return Ok(xml.to_string());
    }

    let replacements = find_replacements(xml, updates)?;
    if replacements.is_empty() {
        return Ok(xml.to_string());
    }

    Ok(apply_replacements(xml, &replacements))
}

struct Replacement {
    start: usize,
    end: usize,
    new_value: String,
}

fn find_replacements(
    xml: &str,
    updates: &HashMap<String, String>,
) -> Result<Vec<Replacement>> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let mut reader = Reader::from_str(xml);
    let mut replacements = Vec::new();
    let mut path_stack: Vec<String> = Vec::new();
    let mut in_properties = false;
    let mut current_prop: Option<String> = None;
    let mut text_start: Option<usize> = None;

    loop {
        let pos = reader.buffer_position();
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(&e);
                if path_stack.len() == 1 && name == "properties" {
                    in_properties = true;
                }
                if in_properties && path_stack.len() >= 2 {
                    if updates.contains_key(&name) {
                        current_prop = Some(name.clone());
                    }
                }
                path_stack.push(name);
                text_start = Some(reader.buffer_position());
            }
            Ok(Event::End(_)) => {
                if let Some(prop_name) = current_prop.take() {
                    if let (Some(start), Some(new_val)) =
                        (text_start, updates.get(&prop_name))
                    {
                        let end = pos;
                        replacements.push(Replacement {
                            start,
                            end,
                            new_value: new_val.clone(),
                        });
                    }
                }
                let popped = path_stack.pop().unwrap_or_default();
                if popped == "properties" {
                    in_properties = false;
                }
                text_start = None;
            }
            Ok(Event::Text(_)) => {}
            Ok(Event::Eof) => break,
            Err(e) => anyhow::bail!("XML parse error: {e}"),
            _ => {}
        }
    }

    Ok(replacements)
}

fn apply_replacements(xml: &str, replacements: &[Replacement]) -> String {
    let mut result = String::with_capacity(xml.len());
    let mut last_end = 0;

    for r in replacements {
        result.push_str(&xml[last_end..r.start]);
        result.push_str(&r.new_value);
        last_end = r.end;
    }
    result.push_str(&xml[last_end..]);
    result
}

fn local_name(e: &quick_xml::events::BytesStart) -> String {
    let full = String::from_utf8_lossy(e.name().as_ref()).to_string();
    full.split(':').next_back().unwrap_or(&full).to_string()
}
```

Note: The `local_name` function duplicates the one in `pom.rs`. That's intentional for now — it's a 3-line function and extracting it to a shared location would couple these modules unnecessarily. If a third caller appears, extract then.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test maven::pom_writer::tests -- --nocapture`
Expected: All 9 tests PASS

- [ ] **Step 5: Register the module**

Add `pub mod pom_writer;` to `src/maven/mod.rs`.

- [ ] **Step 6: Run full test suite**

Run: `cargo test`
Expected: All 143+ tests PASS, no regressions

- [ ] **Step 7: Commit**

```bash
git add src/maven/pom_writer.rs src/maven/mod.rs
git commit -m "feat: add POM property writer with format-preserving updates"
```

---

### Task 2: Maven File-Level Update Logic

**Files:**
- Create: `src/maven/updater.rs`
- Modify: `src/maven/mod.rs` (add `pub mod updater;`)
- Test: `src/maven/updater.rs` (inline `#[cfg(test)]` module)

This module bridges discovery results and the pom_writer. It takes `CheckResult` values that are outdated, maps them back to which POM file and property name to update, reads the file, calls `update_properties`, and writes the result back. It also handles the case where an outdated dependency uses a plain inline version (no property) — those are skipped with a warning, since updating inline versions requires modifying the `<version>` element inside `<dependency>` blocks at potentially multiple locations.

- [ ] **Step 1: Write the failing tests**

```rust
// src/maven/updater.rs

//! Applies version updates to Maven POM files.
//!
//! Maps outdated `CheckResult` values back to POM property names,
//! groups updates by file, and uses `pom_writer::update_properties`
//! for format-preserving rewrites.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::maven::pom_writer;
use crate::registry::{CheckResult, CheckStatus, Ecosystem};

/// Summary of what was updated.
#[derive(Debug, Clone)]
pub struct UpdateResult {
    pub property: String,
    pub old_version: String,
    pub new_version: String,
    pub pom_path: PathBuf,
}

/// Applies updates to POM files for all outdated Maven check results.
///
/// Only updates managed properties (those with `${...}` references).
/// Plain inline versions are skipped — returns them separately.
pub fn apply_updates(
    root: &Path,
    results: &[CheckResult],
) -> Result<(Vec<UpdateResult>, Vec<String>)> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{CheckId, CheckerKind};
    use std::fs;
    use tempfile::TempDir;

    fn write_pom(dir: &Path, content: &str) -> PathBuf {
        let pom_path = dir.join("pom.xml");
        fs::write(&pom_path, content).unwrap();
        pom_path
    }

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
        let tmp = TempDir::new().unwrap();
        let pom_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <properties>
        <version.junit>5.10.0</version.junit>
    </properties>
</project>"#;
        write_pom(tmp.path(), pom_content);

        let results = vec![outdated_result(
            "version.junit",
            "org.junit.jupiter:junit-jupiter",
            "5.10.0",
            "5.12.0",
            "pom.xml",
            true,
        )];

        let (updated, skipped) = apply_updates(tmp.path(), &results).unwrap();
        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].property, "version.junit");
        assert_eq!(updated[0].new_version, "5.12.0");
        assert!(skipped.is_empty());

        let written = fs::read_to_string(tmp.path().join("pom.xml")).unwrap();
        assert!(written.contains("<version.junit>5.12.0</version.junit>"));
    }

    #[test]
    fn skips_unmanaged_inline_versions() {
        let tmp = TempDir::new().unwrap();
        let pom_content = r#"<project>
    <dependencies>
        <dependency>
            <groupId>com.google.guava</groupId>
            <artifactId>guava</artifactId>
            <version>33.0.0-jre</version>
        </dependency>
    </dependencies>
</project>"#;
        write_pom(tmp.path(), pom_content);

        let results = vec![outdated_result(
            "com.google.guava:guava",
            "com.google.guava:guava",
            "33.0.0-jre",
            "33.4.0-jre",
            "pom.xml",
            false,
        )];

        let (updated, skipped) = apply_updates(tmp.path(), &results).unwrap();
        assert!(updated.is_empty());
        assert_eq!(skipped.len(), 1);
        assert!(skipped[0].contains("guava"));
    }

    #[test]
    fn skips_non_outdated_results() {
        let tmp = TempDir::new().unwrap();
        let pom_content = r#"<project>
    <properties>
        <version.junit>5.12.0</version.junit>
    </properties>
</project>"#;
        write_pom(tmp.path(), pom_content);

        let up_to_date = CheckResult::checked(
            CheckId::new(
                Ecosystem::Maven,
                CheckerKind::Dependency,
                "version.junit".to_string(),
                Some("org.junit.jupiter:junit-jupiter".to_string()),
                "pom.xml".to_string(),
            ),
            "5.12.0".to_string(),
            "5.12.0".to_string(),
            false,
        );

        let (updated, skipped) = apply_updates(tmp.path(), &[up_to_date]).unwrap();
        assert!(updated.is_empty());
        assert!(skipped.is_empty());
    }

    #[test]
    fn skips_non_maven_results() {
        let tmp = TempDir::new().unwrap();
        write_pom(tmp.path(), "<project></project>");

        let npm_result = CheckResult::checked(
            CheckId::new(
                Ecosystem::Npm,
                CheckerKind::NpmDep,
                "react".to_string(),
                Some("react".to_string()),
                "package.json".to_string(),
            ),
            "18.0.0".to_string(),
            "19.0.0".to_string(),
            true,
        );

        let (updated, skipped) = apply_updates(tmp.path(), &[npm_result]).unwrap();
        assert!(updated.is_empty());
        assert!(skipped.is_empty());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test maven::updater::tests -- -v`
Expected: FAIL — `todo!()`

- [ ] **Step 3: Implement `apply_updates`**

```rust
pub fn apply_updates(
    root: &Path,
    results: &[CheckResult],
) -> Result<(Vec<UpdateResult>, Vec<String>)> {
    let mut updated = Vec::new();
    let mut skipped = Vec::new();

    // Group outdated Maven results by source POM file
    let mut updates_by_pom: HashMap<PathBuf, HashMap<String, (String, String)>> = HashMap::new();

    for result in results {
        if result.ecosystem() != Ecosystem::Maven {
            continue;
        }
        let CheckStatus::Outdated { latest } = &result.status else {
            continue;
        };

        if !result.has_version_property() {
            let artifact = result.artifact().unwrap_or(result.property_name());
            skipped.push(format!(
                "{} (inline version, update manually)",
                artifact
            ));
            continue;
        }

        let pom_path = root.join(result.source());
        updates_by_pom
            .entry(pom_path)
            .or_default()
            .insert(
                result.property_name().to_string(),
                (result.current_version.clone(), latest.clone()),
            );
    }

    for (pom_path, props) in &updates_by_pom {
        let xml = std::fs::read_to_string(pom_path)
            .with_context(|| format!("Failed to read {}", pom_path.display()))?;

        let new_values: HashMap<String, String> = props
            .iter()
            .map(|(name, (_, new_ver))| (name.clone(), new_ver.clone()))
            .collect();

        let new_xml = pom_writer::update_properties(&xml, &new_values)
            .with_context(|| format!("Failed to update {}", pom_path.display()))?;

        std::fs::write(pom_path, &new_xml)
            .with_context(|| format!("Failed to write {}", pom_path.display()))?;

        for (name, (old_ver, new_ver)) in props {
            updated.push(UpdateResult {
                property: name.clone(),
                old_version: old_ver.clone(),
                new_version: new_ver.clone(),
                pom_path: pom_path.clone(),
            });
        }
    }

    updated.sort_by(|a, b| a.property.cmp(&b.property));
    Ok((updated, skipped))
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test maven::updater::tests -- --nocapture`
Expected: All 4 tests PASS

- [ ] **Step 5: Register the module**

Add `pub mod updater;` to `src/maven/mod.rs`.

- [ ] **Step 6: Run full test suite**

Run: `cargo test`
Expected: All tests PASS

- [ ] **Step 7: Commit**

```bash
git add src/maven/updater.rs src/maven/mod.rs
git commit -m "feat: add Maven updater bridging check results to POM writes"
```

---

### Task 3: npm Update via Package Manager Delegation

**Files:**
- Modify: `src/npm/mod.rs` (add `update_packages` to `PackageManagerChecker` trait)
- Modify: `src/npm/pm_npm.rs` (implement `update_packages`)
- Modify: `src/npm/pm_pnpm.rs` (implement `update_packages`)
- Modify: `src/npm/pm_yarn.rs` (implement `update_packages`)
- Modify: `src/npm/pm_bun.rs` (implement `update_packages`)
- Create: `src/npm/updater.rs` (orchestrates npm updates)
- Test: `src/npm/updater.rs` (inline `#[cfg(test)]` module — limited since it needs real PMs)

The npm update is simpler: delegate to the package manager's native update command. Each PM gets an `update_packages` method on the trait.

- [ ] **Step 1: Add `update_packages` to the trait**

Modify `src/npm/mod.rs` — add the method to `PackageManagerChecker`:

```rust
/// Trait for package-manager-specific operations: listing installed packages,
/// querying for outdated packages, and updating packages.
pub trait PackageManagerChecker {
    /// Lists installed packages as `(name, version, is_dev)` tuples.
    async fn list_packages(&self, dir: &Path) -> Result<Vec<(String, String, bool)>>;
    /// Queries for outdated packages, returning a map of package name to outdated info.
    async fn outdated_packages(&self, dir: &Path) -> Result<HashMap<String, OutdatedEntry>>;
    /// Updates all outdated packages using the PM's native update command.
    async fn update_packages(&self, dir: &Path) -> Result<String>;
}
```

- [ ] **Step 2: Implement `update_packages` for npm**

Add to `src/npm/pm_npm.rs`:

```rust
async fn update_packages(&self, dir: &Path) -> Result<String> {
    let output = Command::new("npm")
        .args(["update"])
        .current_dir(dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .with_context(|| format!("Failed to run 'npm update' in {}", dir.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("npm update failed in {}: {}", dir.display(), stderr.trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
```

- [ ] **Step 3: Implement `update_packages` for pnpm**

Add to `src/npm/pm_pnpm.rs`:

```rust
async fn update_packages(&self, dir: &Path) -> Result<String> {
    let output = Command::new("pnpm")
        .args(["update"])
        .current_dir(dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .with_context(|| format!("Failed to run 'pnpm update' in {}", dir.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("pnpm update failed in {}: {}", dir.display(), stderr.trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
```

- [ ] **Step 4: Implement `update_packages` for yarn**

Add to `src/npm/pm_yarn.rs`:

```rust
async fn update_packages(&self, dir: &Path) -> Result<String> {
    let output = Command::new("yarn")
        .args(["upgrade"])
        .current_dir(dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .with_context(|| format!("Failed to run 'yarn upgrade' in {}", dir.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("yarn upgrade failed in {}: {}", dir.display(), stderr.trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
```

- [ ] **Step 5: Implement `update_packages` for bun**

Add to `src/npm/pm_bun.rs`:

```rust
async fn update_packages(&self, dir: &Path) -> Result<String> {
    let output = Command::new("bun")
        .args(["update"])
        .current_dir(dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .with_context(|| format!("Failed to run 'bun update' in {}", dir.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("bun update failed in {}: {}", dir.display(), stderr.trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
```

- [ ] **Step 6: Create the npm updater orchestrator**

```rust
// src/npm/updater.rs

//! Orchestrates npm ecosystem updates by delegating to each project's
//! package manager native update command.

use std::path::Path;

use anyhow::Result;

use super::discovery::NpmProject;
use super::{PackageManager, PackageManagerChecker, pm_bun, pm_npm, pm_pnpm, pm_yarn};

/// Summary of an npm project update.
#[derive(Debug)]
pub struct NpmUpdateResult {
    pub project_name: String,
    pub package_manager: PackageManager,
    pub success: bool,
    pub message: String,
}

/// Runs the native update command for a single npm project.
pub async fn update_project(project: &NpmProject, root: &Path) -> NpmUpdateResult {
    let relative = project
        .path
        .strip_prefix(root)
        .unwrap_or(&project.path)
        .display()
        .to_string();

    let result = match project.package_manager {
        PackageManager::Npm => pm_npm::Npm.update_packages(&project.path).await,
        PackageManager::Pnpm => pm_pnpm::Pnpm.update_packages(&project.path).await,
        PackageManager::Yarn => pm_yarn::Yarn.update_packages(&project.path).await,
        PackageManager::Bun => pm_bun::Bun.update_packages(&project.path).await,
    };

    match result {
        Ok(output) => NpmUpdateResult {
            project_name: if relative.is_empty() {
                project.name.clone()
            } else {
                relative
            },
            package_manager: project.package_manager,
            success: true,
            message: output,
        },
        Err(e) => NpmUpdateResult {
            project_name: if relative.is_empty() {
                project.name.clone()
            } else {
                relative
            },
            package_manager: project.package_manager,
            success: false,
            message: e.to_string(),
        },
    }
}
```

- [ ] **Step 7: Register the module**

Add `pub mod updater;` to `src/npm/mod.rs`.

- [ ] **Step 8: Run full test suite**

Run: `cargo test`
Expected: All tests PASS (the new trait methods compile, existing tests still pass)

- [ ] **Step 9: Commit**

```bash
git add src/npm/mod.rs src/npm/pm_npm.rs src/npm/pm_pnpm.rs src/npm/pm_yarn.rs src/npm/pm_bun.rs src/npm/updater.rs
git commit -m "feat: add npm update delegation to native PM commands"
```

---

### Task 4: Wire Up the `update` Subcommand

**Files:**
- Modify: `src/command/update.rs` (replace stub with real implementation)
- Modify: `src/app.rs` (add CLI flags to `update_args`)
- Modify: `src/main.rs` (make `update` dispatch async)

The update command flow:
1. Run the check pipeline (discovery → check) to find outdated dependencies
2. Apply Maven POM updates via `maven::updater::apply_updates`
3. Run npm PM update commands via `npm::updater::update_project`
4. Report what was updated

- [ ] **Step 1: Add CLI flags to `update_args` in `app.rs`**

Replace the existing `update_args` function:

```rust
fn update_args(cmd: Command) -> Command {
    cmd.arg(
        Arg::new("path")
            .default_value(".")
            .help("Path to the project root (auto-detects ecosystems)"),
    )
    .arg(
        Arg::new("stable")
            .long("stable")
            .visible_alias("releases-only")
            .action(ArgAction::SetTrue)
            .help("Exclude pre-release versions (alpha, beta, CR, RC, milestone)"),
    )
    .arg(
        Arg::new("maven")
            .long("maven")
            .action(ArgAction::SetTrue)
            .conflicts_with("npm")
            .help("Only update Maven ecosystem"),
    )
    .arg(
        Arg::new("npm")
            .long("npm")
            .action(ArgAction::SetTrue)
            .help("Only update npm ecosystem"),
    )
    .arg(
        Arg::new("dry-run")
            .long("dry-run")
            .action(ArgAction::SetTrue)
            .help("Show what would be updated without making changes"),
    )
}
```

- [ ] **Step 2: Implement the `update` command**

Replace `src/command/update.rs`:

```rust
//! The `update` subcommand: updates outdated dependencies in place.
//!
//! For Maven, rewrites `<properties>` values in POM files.
//! For npm, delegates to the detected package manager's native update command.

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use clap::ArgMatches;
use console::style;
use indicatif::ProgressBar;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::app;
use crate::constants::MAX_CONCURRENT_REQUESTS;
use crate::npm::discovery::NpmProject;
use crate::progress;
use crate::registry::{CheckResult, Ecosystem};

/// Entry point for the `update` subcommand.
pub async fn update(matches: &ArgMatches) -> Result<()> {
    let path = app::path_argument(matches);
    let json = app::is_json(matches);
    let stable = matches.get_flag("stable");
    let dry_run = matches.get_flag("dry-run");
    let maven_only = matches.get_flag("maven");
    let npm_only = matches.get_flag("npm");

    let root = path.canonicalize().unwrap_or_else(|_| path.clone());

    let do_maven = !npm_only && root.join("pom.xml").exists();
    let do_npm = !maven_only;

    // Phase 1: Check for outdated dependencies
    let check_results = run_checks(&root, do_maven, do_npm, stable, json).await?;

    let maven_outdated: Vec<&CheckResult> = check_results
        .iter()
        .filter(|r| r.ecosystem() == Ecosystem::Maven && r.is_outdated())
        .collect();
    let npm_projects = if do_npm {
        crate::npm::discovery::discover(&root)
    } else {
        Vec::new()
    };

    let has_maven_updates = !maven_outdated.is_empty();
    let has_npm_updates = !npm_projects.is_empty()
        && check_results
            .iter()
            .any(|r| r.ecosystem() == Ecosystem::Npm && r.is_outdated());

    if !has_maven_updates && !has_npm_updates {
        if json {
            println!("[]");
        } else {
            println!("{}", style("All dependencies are up to date.").green());
        }
        return Ok(());
    }

    if dry_run {
        print_dry_run(&check_results, json);
        return Ok(());
    }

    // Phase 2: Apply updates
    let mut all_json_results: Vec<serde_json::Value> = Vec::new();

    if do_maven && has_maven_updates {
        let (updated, skipped) =
            crate::maven::updater::apply_updates(&root, &check_results)?;

        if json {
            for u in &updated {
                all_json_results.push(serde_json::json!({
                    "ecosystem": "maven",
                    "property": u.property,
                    "old_version": u.old_version,
                    "new_version": u.new_version,
                    "source": u.pom_path.strip_prefix(&root)
                        .unwrap_or(&u.pom_path)
                        .display()
                        .to_string(),
                    "status": "updated"
                }));
            }
            for s in &skipped {
                all_json_results.push(serde_json::json!({
                    "ecosystem": "maven",
                    "message": s,
                    "status": "skipped"
                }));
            }
        } else {
            for u in &updated {
                println!(
                    "  {} {} {} {} {}",
                    style("\u{2713}").green().bold(),
                    style(&u.property).cyan(),
                    style(&u.old_version).dim(),
                    style("\u{2192}").yellow(),
                    style(&u.new_version).green()
                );
            }
            for s in &skipped {
                println!("  {} {}", style("-").dim(), style(s).dim());
            }
        }
    }

    if do_npm && has_npm_updates {
        let npm_results = run_npm_updates(&npm_projects, &root, json).await;

        if json {
            for r in &npm_results {
                all_json_results.push(serde_json::json!({
                    "ecosystem": "npm",
                    "project": r.project_name,
                    "package_manager": r.package_manager.to_string(),
                    "status": if r.success { "updated" } else { "error" },
                    "message": r.message.trim()
                }));
            }
        } else {
            for r in &npm_results {
                if r.success {
                    println!(
                        "  {} {} ({})",
                        style("\u{2713}").green().bold(),
                        style(&r.project_name).blue(),
                        r.package_manager
                    );
                } else {
                    println!(
                        "  {} {} ({}): {}",
                        style("\u{2717}").red().bold(),
                        style(&r.project_name).blue(),
                        r.package_manager,
                        style(&r.message).red()
                    );
                }
            }
        }
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&all_json_results)
                .unwrap_or_else(|_| "[]".to_string())
        );
    }

    Ok(())
}

async fn run_checks(
    root: &Path,
    do_maven: bool,
    do_npm: bool,
    stable: bool,
    json: bool,
) -> Result<Vec<CheckResult>> {
    let maven_prepared = if do_maven {
        Some(crate::maven::checker::discover(root, stable)?)
    } else {
        None
    };
    let npm_projects = if do_npm {
        crate::npm::discovery::discover(root)
    } else {
        Vec::new()
    };

    let maven_count = maven_prepared.as_ref().map_or(0, |p| p.count());
    let npm_count = npm_projects.len();
    let total = maven_count + npm_count;

    if total == 0 {
        return Ok(Vec::new());
    }

    let bar = if json {
        ProgressBar::hidden()
    } else {
        progress::bar(total as u64)
    };

    let mut join_set: JoinSet<Vec<CheckResult>> = JoinSet::new();

    if let Some(prepared) = maven_prepared {
        let root = root.to_path_buf();
        let bar = bar.clone();
        join_set.spawn(async move { crate::maven::checker::check(&root, prepared, &bar).await });
    }

    crate::command::check::spawn_npm_checks(&mut join_set, npm_projects, root, &bar);

    let results: Vec<CheckResult> = join_set.join_all().await.into_iter().flatten().collect();
    bar.finish_and_clear();

    Ok(results)
}

async fn run_npm_updates(
    projects: &[NpmProject],
    root: &Path,
    json: bool,
) -> Vec<crate::npm::updater::NpmUpdateResult> {
    let bar = if json {
        ProgressBar::hidden()
    } else {
        progress::bar(projects.len() as u64)
    };

    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS));
    let mut join_set = JoinSet::new();

    for project in projects {
        let project = project.clone();
        let root = root.to_path_buf();
        let semaphore = Arc::clone(&semaphore);
        let bar = bar.clone();
        join_set.spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            bar.set_message(format!("{} ({})", project.name, project.package_manager));
            let result = crate::npm::updater::update_project(&project, &root).await;
            bar.inc(1);
            result
        });
    }

    let results = join_set.join_all().await;
    bar.finish_and_clear();
    results
}

fn print_dry_run(results: &[CheckResult], json: bool) {
    let outdated: Vec<&CheckResult> = results.iter().filter(|r| r.is_outdated()).collect();

    if json {
        let json_results: Vec<serde_json::Value> = outdated
            .iter()
            .map(|r| {
                serde_json::json!({
                    "ecosystem": r.ecosystem().to_string().to_lowercase(),
                    "property": r.property_name(),
                    "current": r.current_version,
                    "latest": r.latest_version().unwrap_or(""),
                    "status": "would_update"
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&json_results)
                .unwrap_or_else(|_| "[]".to_string())
        );
    } else {
        println!("{}", style("Dry run — no changes made:").bold());
        for r in &outdated {
            let name = r.artifact().unwrap_or(r.property_name());
            println!(
                "  {} {} {} {} {}",
                style("\u{2192}").yellow(),
                style(name).cyan(),
                style(&r.current_version).dim(),
                style("\u{2192}").yellow(),
                style(r.latest_version().unwrap_or("?")).green()
            );
        }
    }
}
```

- [ ] **Step 3: Make `spawn_npm_checks` public**

In `src/command/check.rs`, change `fn spawn_npm_checks` to `pub fn spawn_npm_checks` so `update.rs` can reuse it.

- [ ] **Step 4: Make `NpmProject` cloneable**

In `src/npm/discovery.rs`, add `#[derive(Clone)]` to `NpmProject` (needed by the update command's JoinSet).

- [ ] **Step 5: Update `main.rs` to dispatch `update` as async**

Change the `update` dispatch line in `src/main.rs`:

```rust
Some(("update", m)) => command::update::update(m).await,
```

- [ ] **Step 6: Run the full test suite**

Run: `cargo test`
Expected: All tests PASS. The existing `update_stub_returns_not_implemented_json` test in `cli_test.rs` will now fail because `update` is no longer a stub — fix it in the next task.

- [ ] **Step 7: Commit**

```bash
git add src/command/update.rs src/command/check.rs src/app.rs src/main.rs src/npm/discovery.rs
git commit -m "feat: implement update subcommand with Maven POM writes and npm delegation"
```

---

### Task 5: Update Integration Tests

**Files:**
- Modify: `tests/cli_test.rs` (update existing stub test, add new update tests)
- Create: `tests/fixtures/update-test/pom.xml` (fixture for update tests)

- [ ] **Step 1: Create the update test fixture**

```xml
<!-- tests/fixtures/update-test/pom.xml -->
<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
    <modelVersion>4.0.0</modelVersion>
    <groupId>org.example</groupId>
    <artifactId>update-test</artifactId>
    <version>1.0.0</version>

    <properties>
        <!-- Intentionally old versions for update testing -->
        <version.junit>5.10.0</version.junit>
        <version.compiler.plugin>3.11.0</version.compiler.plugin>
    </properties>

    <dependencyManagement>
        <dependencies>
            <dependency>
                <groupId>org.junit.jupiter</groupId>
                <artifactId>junit-jupiter</artifactId>
                <version>${version.junit}</version>
            </dependency>
        </dependencies>
    </dependencyManagement>

    <build>
        <pluginManagement>
            <plugins>
                <plugin>
                    <groupId>org.apache.maven.plugins</groupId>
                    <artifactId>maven-compiler-plugin</artifactId>
                    <version>${version.compiler.plugin}</version>
                </plugin>
            </plugins>
        </pluginManagement>
    </build>
</project>
```

- [ ] **Step 2: Update existing stub test and add new tests**

Replace `update_stub_returns_not_implemented_json` and add new tests in `tests/cli_test.rs`:

```rust
#[test]
fn update_dry_run_shows_would_update() {
    let output = depup()
        .arg("update")
        .arg("--json")
        .arg("--dry-run")
        .arg(&fixture_dir("multi-module"))
        .output()
        .expect("Failed to run depup");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).expect("Invalid JSON output");

    for result in &results {
        assert_eq!(
            result["status"].as_str().unwrap(),
            "would_update",
            "Dry run should report would_update status"
        );
    }
}

#[test]
fn update_maven_modifies_pom() {
    // Copy fixture to a temp dir so we can safely modify it
    let tmp = tempfile::TempDir::new().unwrap();
    let fixture = fixture_dir("update-test");
    let pom_src = fixture.join("pom.xml");
    let pom_dst = tmp.path().join("pom.xml");
    std::fs::copy(&pom_src, &pom_dst).unwrap();

    let output = depup()
        .arg("update")
        .arg("--json")
        .arg(tmp.path())
        .output()
        .expect("Failed to run depup");

    assert!(output.status.success());

    // Verify the POM was modified
    let pom_content = std::fs::read_to_string(&pom_dst).unwrap();
    // The original version was 5.10.0 — it should now be different (newer)
    assert!(
        !pom_content.contains("<version.junit>5.10.0</version.junit>"),
        "POM should have been updated with a newer junit version"
    );
    // Verify formatting is preserved
    assert!(pom_content.contains("<!-- Intentionally old versions for update testing -->"));
    assert!(pom_content.contains("xmlns=\"http://maven.apache.org/POM/4.0.0\""));
}

#[test]
fn update_preserves_pom_formatting() {
    let tmp = tempfile::TempDir::new().unwrap();
    let fixture = fixture_dir("update-test");
    let pom_src = fixture.join("pom.xml");
    let pom_dst = tmp.path().join("pom.xml");
    std::fs::copy(&pom_src, &pom_dst).unwrap();

    let original = std::fs::read_to_string(&pom_dst).unwrap();

    let output = depup()
        .arg("update")
        .arg(tmp.path())
        .output()
        .expect("Failed to run depup");

    assert!(output.status.success());

    let updated = std::fs::read_to_string(&pom_dst).unwrap();
    // Line count should be preserved (only values change, not structure)
    assert_eq!(
        original.lines().count(),
        updated.lines().count(),
        "Update should preserve POM line count"
    );
}

#[test]
fn update_all_current_reports_nothing() {
    // Use a path with no project files
    let tmp = tempfile::TempDir::new().unwrap();
    let output = depup()
        .arg("update")
        .arg("--json")
        .arg(tmp.path())
        .output()
        .expect("Failed to run depup");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "[]");
}

#[test]
fn update_maven_only_flag() {
    let output = depup()
        .arg("update")
        .arg("--json")
        .arg("--dry-run")
        .arg("--maven")
        .arg(&fixture_dir("multi-module"))
        .output()
        .expect("Failed to run depup");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let results: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).expect("Invalid JSON output");

    for result in &results {
        assert_eq!(result["ecosystem"].as_str().unwrap(), "maven");
    }
}
```

- [ ] **Step 3: Run the integration tests**

Run: `cargo test --test cli_test -- -v`
Expected: All tests PASS including the new ones

- [ ] **Step 4: Run full test suite**

Run: `cargo test`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add tests/cli_test.rs tests/fixtures/update-test/
git commit -m "test: add integration tests for update subcommand"
```

---

### Task 6: Update Documentation and Changelog

**Files:**
- Modify: `README.md` (document update subcommand)
- Modify: `CLAUDE.md` (update architecture description)
- Modify: `CHANGELOG.md` (add update entries)

- [ ] **Step 1: Update README.md**

Add `update` to the usage examples and command reference. Add a section showing:

```
depup update /path               # update all outdated dependencies
depup update --dry-run /path     # show what would be updated
depup update --maven /path       # update Maven dependencies only
depup update --npm /path         # update npm dependencies only
depup update --stable /path      # only update to stable releases
depup update --json /path        # JSON output for scripting
```

- [ ] **Step 2: Update CLAUDE.md architecture section**

Add documentation for the new modules:
- `src/maven/pom_writer.rs` — Surgical POM property value replacement
- `src/maven/updater.rs` — Maps check results to POM file updates
- `src/npm/updater.rs` — Delegates to PM native update commands
- Update the `update.rs` command description

- [ ] **Step 3: Update CHANGELOG.md**

Add under `## [Unreleased]`:

```markdown
### Added

- `update` subcommand for updating outdated dependencies
- Maven: format-preserving POM property updates (preserves comments, whitespace, indentation)
- npm: delegates to native package manager update commands (npm, pnpm, yarn, bun)
- `--dry-run` flag to preview updates without making changes
- `--maven` / `--npm` flags to limit updates to a single ecosystem
- `--stable` flag to only update to stable releases
```

- [ ] **Step 4: Commit**

```bash
git add README.md CLAUDE.md CHANGELOG.md
git commit -m "docs: document update subcommand"
```

---

### Task 7: Final Verification

- [ ] **Step 1: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings

- [ ] **Step 2: Run fmt check**

Run: `cargo fmt -- --check`
Expected: No formatting issues

- [ ] **Step 3: Run full test suite**

Run: `cargo test`
Expected: All tests PASS

- [ ] **Step 4: Manual smoke test**

Run against a real Maven project (e.g., one of your WildFly projects):

```bash
cargo run -- update --dry-run /path/to/maven/project
```

Verify the dry-run output looks correct, then:

```bash
cargo run -- update /path/to/maven/project
```

Verify the POM files were updated correctly with `git diff`.

- [ ] **Step 5: Run `cargo build --release`**

Run: `cargo build --release`
Expected: Clean release build

- [ ] **Step 6: Commit any final fixes**

If any issues were found in the verification steps, fix and commit them.
