//! POM property writer for format-preserving updates.
//!
//! This module provides surgical property replacement without altering formatting,
//! comments, whitespace, or XML structure. Used by the `update` command.

use std::collections::HashMap;

use anyhow::Result;
use quick_xml::Reader;
use quick_xml::events::Event;

use crate::error::DepupError;

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

    let mut reader = Reader::from_str(xml);
    let mut path_stack: Vec<String> = Vec::new();
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();

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
                                        replacements.push((start_pos, end_pos, new_value.clone()));
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

    // If no replacements, return original
    if replacements.is_empty() {
        return Ok(xml.to_string());
    }

    // Sort replacements by start position (should already be sorted, but ensure it)
    replacements.sort_by_key(|(start, _, _)| *start);

    // Build result by splicing
    let mut result = String::new();
    let mut last_pos = 0;

    for (start, end, new_value) in replacements {
        // Append unchanged portion
        result.push_str(&xml[last_pos..start]);
        // Append new value
        result.push_str(&new_value);
        last_pos = end;
    }

    // Append remainder
    result.push_str(&xml[last_pos..]);

    Ok(result)
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
}
