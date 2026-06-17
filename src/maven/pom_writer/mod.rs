//! POM writer for format-preserving updates.
//!
//! Provides surgical XML replacement without altering formatting,
//! comments, whitespace, or XML structure. Used by the `update` command.
//!
//! - [`properties`] — updates `<properties>` values
//! - [`inline`] — updates `<version>` elements in dependency/plugin blocks

mod inline;
mod properties;

pub use inline::{InlineVersionUpdate, update_inline_versions};
pub use properties::update_properties;

use crate::error::DepupError;

/// Represents a single text replacement in XML content.
pub(super) struct Replacement {
    pub start: usize,
    pub end: usize,
    pub new_value: String,
}

/// Applies multiple replacements to XML content while preserving formatting.
///
/// Replacements are sorted by start position and applied in order, with no overlaps allowed.
pub(super) fn apply_replacements(xml: &str, mut replacements: Vec<Replacement>) -> String {
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

/// Strips namespace prefix from an XML element name.
pub(super) fn local_name(name: quick_xml::name::QName) -> String {
    String::from_utf8_lossy(name.local_name().as_ref()).to_string()
}

/// Creates a POM parse error.
pub(super) fn parse_error(message: &str) -> anyhow::Error {
    DepupError::pom_parse_failed("POM XML", message).into()
}

/// Skips to the closing tag of the current element, tracking nested depth.
/// Returns an error on unexpected EOF or XML parse errors.
pub(super) fn skip_element(
    reader: &mut quick_xml::Reader<&[u8]>,
    element_name: &str,
) -> anyhow::Result<()> {
    let mut depth: u32 = 1;
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(_)) => depth += 1,
            Ok(quick_xml::events::Event::End(_)) => {
                depth -= 1;
                if depth == 0 {
                    return Ok(());
                }
            }
            Ok(quick_xml::events::Event::Eof) => {
                return Err(parse_error(&format!(
                    "Unexpected EOF while skipping element '{element_name}'"
                )));
            }
            Err(e) => return Err(parse_error(&format!("XML parse error: {e}"))),
            _ => {}
        }
    }
}
