//! POM property writer for format-preserving updates.
//!
//! This module provides surgical property replacement without altering formatting,
//! comments, whitespace, or XML structure. Used by the `update` command.

use std::collections::HashMap;

use anyhow::Result;
use quick_xml::Reader;
use quick_xml::events::Event;

use crate::error::DepupError;

/// Represents a single text replacement in XML content.
struct Replacement {
    start: usize,
    end: usize,
    new_value: String,
}

/// Applies multiple replacements to XML content while preserving formatting.
///
/// Replacements are sorted by start position and applied in order, with no overlaps allowed.
fn apply_replacements(xml: &str, mut replacements: Vec<Replacement>) -> String {
    if replacements.is_empty() {
        return xml.to_string();
    }
    replacements.sort_by_key(|r| r.start);
    let mut result = String::with_capacity(xml.len());
    let mut last_pos = 0;
    for r in &replacements {
        result.push_str(&xml[last_pos..r.start]);
        result.push_str(&r.new_value);
        last_pos = r.end;
    }
    result.push_str(&xml[last_pos..]);
    result
}

/// Update information for a single inline version.
#[allow(dead_code)] // Will be used by Maven updater in future PR
#[derive(Debug, Clone)]
pub struct InlineVersionUpdate {
    pub group_id: String,
    pub artifact_id: String,
    pub new_version: String,
}

/// Updates property values in a POM XML string while preserving formatting.
///
/// This function surgically replaces property values inside `<properties>` blocks
/// without altering whitespace, comments, indentation, or structure. It uses
/// quick-xml only to locate element boundaries, then performs string splicing.
///
/// # Arguments
/// * `xml` - The POM XML content as a string
/// * `updates` - Map of property names to new values
///
/// # Returns
/// The updated XML string with properties replaced, or the original if updates is empty
pub fn update_properties(xml: &str, updates: &HashMap<String, String>) -> Result<String> {
    if updates.is_empty() {
        return Ok(xml.to_string());
    }
    let replacements = find_property_replacements(xml, updates)?;
    Ok(apply_replacements(xml, replacements))
}

/// Finds property replacements in a POM XML string.
fn find_property_replacements(
    xml: &str,
    updates: &HashMap<String, String>,
) -> Result<Vec<Replacement>> {
    let mut reader = Reader::from_str(xml);
    let mut path_stack: Vec<String> = Vec::new();
    let mut replacements: Vec<Replacement> = Vec::new();

    loop {
        let _pos_before_event = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name());
                let start_pos_after_tag = reader.buffer_position() as usize;
                path_stack.push(name.clone());

                // Check if we're at depth 3 inside <properties>
                if path_stack.len() == 3
                    && path_stack[0] == "project"
                    && path_stack[1] == "properties"
                {
                    if let Some(new_value) = updates.get(&name) {
                        // Record start position (right after the opening tag '>')
                        let start_pos = start_pos_after_tag;

                        // Read events until we hit the matching End
                        let mut depth = 1;
                        loop {
                            let pos_before_inner = reader.buffer_position() as usize;
                            match reader.read_event() {
                                Ok(Event::Start(_)) => {
                                    depth += 1;
                                }
                                Ok(Event::End(_)) => {
                                    depth -= 1;
                                    if depth == 0 {
                                        // pos_before_inner is right before the '<' of the closing tag
                                        let end_pos = pos_before_inner;
                                        replacements.push(Replacement {
                                            start: start_pos,
                                            end: end_pos,
                                            new_value: new_value.clone(),
                                        });
                                        // Pop from path_stack since we consumed the End event
                                        path_stack.pop();
                                        break;
                                    }
                                }
                                Ok(Event::Eof) => {
                                    return Err(DepupError::pom_parse_failed(
                                        "POM XML",
                                        &format!("Unexpected EOF while reading property {}", name),
                                    )
                                    .into());
                                }
                                Err(e) => {
                                    return Err(DepupError::pom_parse_failed(
                                        "POM XML",
                                        &format!("XML parse error: {}", e),
                                    )
                                    .into());
                                }
                                _ => {}
                            }
                        }
                    } else {
                        // Property not in updates, skip to its End event
                        let mut depth = 1;
                        loop {
                            match reader.read_event() {
                                Ok(Event::Start(_)) => depth += 1,
                                Ok(Event::End(_)) => {
                                    depth -= 1;
                                    if depth == 0 {
                                        path_stack.pop();
                                        break;
                                    }
                                }
                                Ok(Event::Eof) => {
                                    return Err(DepupError::pom_parse_failed(
                                        "POM XML",
                                        "Unexpected EOF",
                                    )
                                    .into());
                                }
                                Err(e) => {
                                    return Err(DepupError::pom_parse_failed(
                                        "POM XML",
                                        &format!("XML parse error: {}", e),
                                    )
                                    .into());
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            Ok(Event::End(_)) => {
                // Only pop if we're not at depth 3 (property elements handle their own popping)
                if path_stack.len() != 3 || path_stack.get(1) != Some(&"properties".to_string()) {
                    path_stack.pop();
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(DepupError::pom_parse_failed(
                    "POM XML",
                    &format!("XML parse error: {}", e),
                )
                .into());
            }
            _ => {}
        }
    }

    Ok(replacements)
}

/// Updates inline dependency and plugin versions in a POM XML string.
///
/// This function surgically replaces `<version>` elements within `<dependency>` and
/// `<plugin>` blocks, matching by groupId and artifactId coordinates. It preserves
/// all formatting, comments, and structure.
///
/// # Arguments
/// * `xml` - The POM XML content as a string
/// * `updates` - Slice of inline version updates with coordinates and new versions
///
/// # Returns
/// The updated XML string with inline versions replaced, or the original if updates is empty
#[allow(dead_code)] // Will be used by Maven updater in future PR
pub fn update_inline_versions(xml: &str, updates: &[InlineVersionUpdate]) -> Result<String> {
    if updates.is_empty() {
        return Ok(xml.to_string());
    }
    let replacements = find_inline_replacements(xml, updates)?;
    Ok(apply_replacements(xml, replacements))
}

/// Finds inline version replacements in dependency and plugin blocks.
#[allow(dead_code)] // Called by update_inline_versions
fn find_inline_replacements(
    xml: &str,
    updates: &[InlineVersionUpdate],
) -> Result<Vec<Replacement>> {
    let mut reader = Reader::from_str(xml);
    let mut path_stack: Vec<String> = Vec::new();
    let mut replacements: Vec<Replacement> = Vec::new();

    // State machine for tracking artifact blocks
    let mut in_artifact_block = false;
    let mut current_group_id: Option<String> = None;
    let mut current_artifact_id: Option<String> = None;
    let mut version_start: Option<usize> = None;
    let mut version_end: Option<usize> = None;
    let mut in_child_element: Option<String> = None;
    let mut child_text = String::new();

    loop {
        let pos_before_event = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name());
                let start_pos_after_tag = reader.buffer_position() as usize;
                path_stack.push(name.clone());

                // Check if we're entering a dependency or plugin block
                if !in_artifact_block {
                    if is_artifact_block(&path_stack) {
                        in_artifact_block = true;
                        current_group_id = None;
                        current_artifact_id = None;
                        version_start = None;
                        version_end = None;
                    }
                } else if !is_nested_block(&path_stack) {
                    // Track child elements (groupId, artifactId, version)
                    in_child_element = Some(name);
                    child_text.clear();
                    if &in_child_element.as_ref().unwrap()[..] == "version" {
                        version_start = Some(start_pos_after_tag);
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name());

                // If we're in a child element, capture its text and record position
                if let Some(ref child_name) = in_child_element {
                    if child_name == &name {
                        match child_name.as_str() {
                            "groupId" => current_group_id = Some(child_text.trim().to_string()),
                            "artifactId" => {
                                current_artifact_id = Some(child_text.trim().to_string())
                            }
                            "version" => version_end = Some(pos_before_event),
                            _ => {}
                        }
                        in_child_element = None;
                        child_text.clear();
                    }
                }

                // Check if we're leaving an artifact block
                if in_artifact_block && is_artifact_element(&name) {
                    // Match against updates
                    if let (Some(gid), Some(aid), Some(v_start), Some(v_end)) = (
                        &current_group_id,
                        &current_artifact_id,
                        version_start,
                        version_end,
                    ) {
                        for update in updates {
                            if update.group_id == *gid && update.artifact_id == *aid {
                                replacements.push(Replacement {
                                    start: v_start,
                                    end: v_end,
                                    new_value: update.new_version.clone(),
                                });
                                break;
                            }
                        }
                    }
                    // Reset state
                    in_artifact_block = false;
                    current_group_id = None;
                    current_artifact_id = None;
                    version_start = None;
                    version_end = None;
                }

                path_stack.pop();
            }
            Ok(Event::Text(e)) => {
                if in_child_element.is_some() {
                    child_text.push_str(&e.unescape().unwrap_or_default());
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(DepupError::pom_parse_failed(
                    "POM XML",
                    &format!("XML parse error: {}", e),
                )
                .into());
            }
            _ => {}
        }
    }

    Ok(replacements)
}

/// Checks if the current path represents a dependency or plugin block.
#[allow(dead_code)] // Called by find_inline_replacements
fn is_artifact_block(path_stack: &[String]) -> bool {
    if path_stack.is_empty() {
        return false;
    }
    let last = &path_stack[path_stack.len() - 1];
    if last == "dependency" {
        path_stack
            .iter()
            .any(|e| e == "dependencies" || e == "dependencyManagement")
    } else if last == "plugin" {
        path_stack
            .iter()
            .any(|e| e == "plugins" || e == "pluginManagement")
    } else {
        false
    }
}

/// Checks if the current path is inside a nested block that should be skipped.
#[allow(dead_code)] // Called by find_inline_replacements
fn is_nested_block(path_stack: &[String]) -> bool {
    path_stack
        .iter()
        .any(|e| e == "exclusions" || e == "configuration")
}

/// Checks if an element name is an artifact element (dependency or plugin).
#[allow(dead_code)] // Called by find_inline_replacements
fn is_artifact_element(name: &str) -> bool {
    name == "dependency" || name == "plugin"
}

/// Strips namespace prefix from an XML element name.
fn local_name(name: quick_xml::name::QName) -> String {
    String::from_utf8_lossy(name.local_name().as_ref()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updates_single_property() {
        let xml = r#"<project>
    <properties>
        <version.foo>1.0.0</version.foo>
        <version.bar>2.0.0</version.bar>
    </properties>
</project>"#;

        let mut updates = HashMap::new();
        updates.insert("version.foo".to_string(), "1.1.0".to_string());

        let result = update_properties(xml, &updates).unwrap();

        assert!(result.contains("<version.foo>1.1.0</version.foo>"));
        assert!(result.contains("<version.bar>2.0.0</version.bar>"));
    }

    #[test]
    fn updates_multiple_properties() {
        let xml = r#"<project>
    <properties>
        <version.foo>1.0.0</version.foo>
        <version.bar>2.0.0</version.bar>
        <version.baz>3.0.0</version.baz>
    </properties>
</project>"#;

        let mut updates = HashMap::new();
        updates.insert("version.foo".to_string(), "1.1.0".to_string());
        updates.insert("version.baz".to_string(), "3.3.0".to_string());

        let result = update_properties(xml, &updates).unwrap();

        assert!(result.contains("<version.foo>1.1.0</version.foo>"));
        assert!(result.contains("<version.bar>2.0.0</version.bar>"));
        assert!(result.contains("<version.baz>3.3.0</version.baz>"));
    }

    #[test]
    fn preserves_comments() {
        let xml = r#"<project>
    <properties>
        <!-- This is a comment -->
        <version.foo>1.0.0</version.foo>
        <!-- Another comment -->
        <version.bar>2.0.0</version.bar>
    </properties>
</project>"#;

        let mut updates = HashMap::new();
        updates.insert("version.foo".to_string(), "1.1.0".to_string());

        let result = update_properties(xml, &updates).unwrap();

        assert!(result.contains("<!-- This is a comment -->"));
        assert!(result.contains("<!-- Another comment -->"));
        assert!(result.contains("<version.foo>1.1.0</version.foo>"));
    }

    #[test]
    fn preserves_indentation_and_whitespace() {
        let xml = r#"<project>
	<properties>
		<version.foo>1.0.0</version.foo>
		<version.bar>2.0.0</version.bar>
	</properties>
</project>"#;

        let mut updates = HashMap::new();
        updates.insert("version.foo".to_string(), "1.1.0".to_string());

        let result = update_properties(xml, &updates).unwrap();

        // Check that tabs are preserved
        assert!(result.contains("\t<properties>"));
        assert!(result.contains("\t\t<version.foo>1.1.0</version.foo>"));
        assert!(result.contains("\t\t<version.bar>2.0.0</version.bar>"));
    }

    #[test]
    fn ignores_properties_not_in_updates() {
        let xml = r#"<project>
    <properties>
        <version.foo>1.0.0</version.foo>
        <version.bar>2.0.0</version.bar>
        <version.baz>3.0.0</version.baz>
    </properties>
</project>"#;

        let mut updates = HashMap::new();
        updates.insert("version.foo".to_string(), "1.1.0".to_string());

        let result = update_properties(xml, &updates).unwrap();

        assert!(result.contains("<version.bar>2.0.0</version.bar>"));
        assert!(result.contains("<version.baz>3.0.0</version.baz>"));
    }

    #[test]
    fn handles_xml_namespaces() {
        let xml = r#"<project xmlns="http://maven.apache.org/POM/4.0.0">
    <properties>
        <version.foo>1.0.0</version.foo>
        <version.bar>2.0.0</version.bar>
    </properties>
</project>"#;

        let mut updates = HashMap::new();
        updates.insert("version.foo".to_string(), "1.1.0".to_string());

        let result = update_properties(xml, &updates).unwrap();

        assert!(result.contains("<version.foo>1.1.0</version.foo>"));
        assert!(result.contains("xmlns=\"http://maven.apache.org/POM/4.0.0\""));
    }

    #[test]
    fn no_updates_returns_unchanged() {
        let xml = r#"<project>
    <properties>
        <version.foo>1.0.0</version.foo>
    </properties>
</project>"#;

        let updates = HashMap::new();
        let result = update_properties(xml, &updates).unwrap();

        assert_eq!(result, xml);
    }

    #[test]
    fn preserves_trailing_newline() {
        let xml = "<project>\n    <properties>\n        <version.foo>1.0.0</version.foo>\n    </properties>\n</project>\n";

        let mut updates = HashMap::new();
        updates.insert("version.foo".to_string(), "1.1.0".to_string());

        let result = update_properties(xml, &updates).unwrap();

        assert!(result.ends_with('\n'));
    }

    #[test]
    fn handles_property_with_whitespace_value() {
        let xml = r#"<project>
    <properties>
        <version.foo>  1.0.0  </version.foo>
    </properties>
</project>"#;

        let mut updates = HashMap::new();
        updates.insert("version.foo".to_string(), "1.1.0".to_string());

        let result = update_properties(xml, &updates).unwrap();

        // The new value replaces the entire text content between tags
        assert!(result.contains("<version.foo>1.1.0</version.foo>"));
    }

    #[test]
    fn updates_inline_dependency_version() {
        let xml = r#"<project>
    <dependencies>
        <dependency>
            <groupId>com.google.guava</groupId>
            <artifactId>guava</artifactId>
            <version>33.0.0-jre</version>
        </dependency>
    </dependencies>
</project>"#;

        let updates = vec![InlineVersionUpdate {
            group_id: "com.google.guava".to_string(),
            artifact_id: "guava".to_string(),
            new_version: "33.4.0-jre".to_string(),
        }];

        let result = update_inline_versions(xml, &updates).unwrap();
        assert!(result.contains("<version>33.4.0-jre</version>"));
        assert!(result.contains("<groupId>com.google.guava</groupId>"));
    }

    #[test]
    fn updates_inline_plugin_version() {
        let xml = r#"<project>
    <build>
        <plugins>
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-compiler-plugin</artifactId>
                <version>3.11.0</version>
            </plugin>
        </plugins>
    </build>
</project>"#;

        let updates = vec![InlineVersionUpdate {
            group_id: "org.apache.maven.plugins".to_string(),
            artifact_id: "maven-compiler-plugin".to_string(),
            new_version: "3.13.0".to_string(),
        }];

        let result = update_inline_versions(xml, &updates).unwrap();
        assert!(result.contains("<version>3.13.0</version>"));
    }

    #[test]
    fn inline_preserves_formatting() {
        let xml = r#"<project>
    <dependencies>
        <!-- Guava library -->
        <dependency>
            <groupId>com.google.guava</groupId>
            <artifactId>guava</artifactId>
            <version>33.0.0-jre</version>
        </dependency>
    </dependencies>
</project>"#;

        let updates = vec![InlineVersionUpdate {
            group_id: "com.google.guava".to_string(),
            artifact_id: "guava".to_string(),
            new_version: "33.4.0-jre".to_string(),
        }];

        let result = update_inline_versions(xml, &updates).unwrap();
        assert!(result.contains("<!-- Guava library -->"));
        assert_eq!(xml.lines().count(), result.lines().count());
    }

    #[test]
    fn inline_matches_by_coordinates() {
        let xml = r#"<project>
    <dependencies>
        <dependency>
            <groupId>com.google.guava</groupId>
            <artifactId>guava</artifactId>
            <version>33.0.0-jre</version>
        </dependency>
        <dependency>
            <groupId>org.junit.jupiter</groupId>
            <artifactId>junit-jupiter</artifactId>
            <version>5.10.0</version>
        </dependency>
    </dependencies>
</project>"#;

        let updates = vec![InlineVersionUpdate {
            group_id: "com.google.guava".to_string(),
            artifact_id: "guava".to_string(),
            new_version: "33.4.0-jre".to_string(),
        }];

        let result = update_inline_versions(xml, &updates).unwrap();
        assert!(result.contains("<version>33.4.0-jre</version>"));
        assert!(result.contains("<version>5.10.0</version>"));
    }

    #[test]
    fn inline_no_match_returns_unchanged() {
        let xml = r#"<project>
    <dependencies>
        <dependency>
            <groupId>com.google.guava</groupId>
            <artifactId>guava</artifactId>
            <version>33.0.0-jre</version>
        </dependency>
    </dependencies>
</project>"#;

        let updates = vec![InlineVersionUpdate {
            group_id: "org.example".to_string(),
            artifact_id: "no-match".to_string(),
            new_version: "1.0.0".to_string(),
        }];

        let result = update_inline_versions(xml, &updates).unwrap();
        assert_eq!(result, xml);
    }

    #[test]
    fn inline_handles_dependency_management() {
        let xml = r#"<project>
    <dependencyManagement>
        <dependencies>
            <dependency>
                <groupId>com.google.guava</groupId>
                <artifactId>guava</artifactId>
                <version>33.0.0-jre</version>
            </dependency>
        </dependencies>
    </dependencyManagement>
</project>"#;

        let updates = vec![InlineVersionUpdate {
            group_id: "com.google.guava".to_string(),
            artifact_id: "guava".to_string(),
            new_version: "33.4.0-jre".to_string(),
        }];

        let result = update_inline_versions(xml, &updates).unwrap();
        assert!(result.contains("<version>33.4.0-jre</version>"));
    }

    #[test]
    fn inline_handles_xml_namespaces() {
        let xml = r#"<project xmlns="http://maven.apache.org/POM/4.0.0">
    <dependencies>
        <dependency>
            <groupId>com.google.guava</groupId>
            <artifactId>guava</artifactId>
            <version>33.0.0-jre</version>
        </dependency>
    </dependencies>
</project>"#;

        let updates = vec![InlineVersionUpdate {
            group_id: "com.google.guava".to_string(),
            artifact_id: "guava".to_string(),
            new_version: "33.4.0-jre".to_string(),
        }];

        let result = update_inline_versions(xml, &updates).unwrap();
        assert!(result.contains("<version>33.4.0-jre</version>"));
        assert!(result.contains("xmlns="));
    }

    #[test]
    fn inline_skips_exclusion_and_configuration_versions() {
        let xml = r#"<project>
    <build>
        <plugins>
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-enforcer-plugin</artifactId>
                <version>3.4.0</version>
                <configuration>
                    <rules>
                        <requireJavaVersion>
                            <version>17</version>
                        </requireJavaVersion>
                    </rules>
                </configuration>
            </plugin>
        </plugins>
    </build>
</project>"#;

        let updates = vec![InlineVersionUpdate {
            group_id: "org.apache.maven.plugins".to_string(),
            artifact_id: "maven-enforcer-plugin".to_string(),
            new_version: "3.5.0".to_string(),
        }];

        let result = update_inline_versions(xml, &updates).unwrap();
        assert!(result.contains(
            "<artifactId>maven-enforcer-plugin</artifactId>\n                <version>3.5.0</version>"
        ));
        assert!(result.contains("<version>17</version>"));
    }
}
