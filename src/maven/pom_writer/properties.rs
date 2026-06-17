//! Property value replacement in `<properties>` blocks.

use std::collections::HashMap;

use anyhow::Result;
use quick_xml::Reader;
use quick_xml::events::Event;

use super::{Replacement, apply_replacements, local_name, parse_error, skip_element};

/// Updates property values in a POM XML string while preserving formatting.
///
/// Surgically replaces property values inside `<properties>` blocks
/// without altering whitespace, comments, indentation, or structure. Uses
/// quick-xml only to locate element boundaries, then performs string splicing.
pub fn update_properties(xml: &str, updates: &HashMap<String, String>) -> Result<String> {
    if updates.is_empty() {
        return Ok(xml.to_string());
    }
    let replacements = find_property_replacements(xml, updates)?;
    Ok(apply_replacements(xml, replacements))
}

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

                if path_stack.len() == 3
                    && path_stack[0] == "project"
                    && path_stack[1] == "properties"
                {
                    if let Some(new_value) = updates.get(&name) {
                        let start_pos = start_pos_after_tag;
                        let mut depth: u32 = 1;
                        loop {
                            let pos_before_inner = reader.buffer_position() as usize;
                            match reader.read_event() {
                                Ok(Event::Start(_)) => {
                                    depth += 1;
                                }
                                Ok(Event::End(_)) => {
                                    depth -= 1;
                                    if depth == 0 {
                                        let end_pos = pos_before_inner;
                                        replacements.push(Replacement {
                                            start: start_pos,
                                            end: end_pos,
                                            new_value: new_value.clone(),
                                        });
                                        path_stack.pop();
                                        break;
                                    }
                                }
                                Ok(Event::Eof) => {
                                    return Err(parse_error(&format!(
                                        "Unexpected EOF while reading property {}",
                                        name
                                    )));
                                }
                                Err(e) => {
                                    return Err(parse_error(&format!("XML parse error: {}", e)));
                                }
                                _ => {}
                            }
                        }
                    } else {
                        skip_element(&mut reader, &name)?;
                        path_stack.pop();
                    }
                }
            }
            Ok(Event::End(_)) => {
                if path_stack.len() != 3 || path_stack.get(1) != Some(&"properties".to_string()) {
                    path_stack.pop();
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

        assert!(result.contains("<version.foo>1.1.0</version.foo>"));
    }
}
