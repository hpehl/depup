//! Inline version replacement in dependency and plugin blocks.

use anyhow::Result;
use quick_xml::Reader;
use quick_xml::events::Event;

use super::{Replacement, apply_replacements, local_name, parse_error};

/// Update information for a single inline version.
#[derive(Debug, Clone)]
pub struct InlineVersionUpdate {
    pub group_id: String,
    pub artifact_id: String,
    pub new_version: String,
}

/// Updates inline dependency and plugin versions in a POM XML string.
///
/// Surgically replaces `<version>` elements within `<dependency>` and
/// `<plugin>` blocks, matching by groupId and artifactId coordinates. Preserves
/// all formatting, comments, and structure.
pub fn update_inline_versions(xml: &str, updates: &[InlineVersionUpdate]) -> Result<String> {
    if updates.is_empty() {
        return Ok(xml.to_string());
    }
    let replacements = find_inline_replacements(xml, updates)?;
    Ok(apply_replacements(xml, replacements))
}

fn find_inline_replacements(
    xml: &str,
    updates: &[InlineVersionUpdate],
) -> Result<Vec<Replacement>> {
    let mut reader = Reader::from_str(xml);
    let mut path_stack: Vec<String> = Vec::new();
    let mut replacements: Vec<Replacement> = Vec::new();

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

                if !in_artifact_block {
                    if is_artifact_block(&path_stack) {
                        in_artifact_block = true;
                        current_group_id = None;
                        current_artifact_id = None;
                        version_start = None;
                        version_end = None;
                    }
                } else if !is_nested_block(&path_stack) {
                    in_child_element = Some(name);
                    child_text.clear();
                    if &in_child_element.as_ref().unwrap()[..] == "version" {
                        version_start = Some(start_pos_after_tag);
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name());

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

                if in_artifact_block && is_artifact_element(&name) {
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
                return Err(parse_error(&format!("XML parse error: {}", e)));
            }
            _ => {}
        }
    }

    Ok(replacements)
}

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

fn is_nested_block(path_stack: &[String]) -> bool {
    path_stack
        .iter()
        .any(|e| e == "exclusions" || e == "configuration")
}

fn is_artifact_element(name: &str) -> bool {
    name == "dependency" || name == "plugin"
}

#[cfg(test)]
mod tests {
    use super::*;

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
