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
pub(crate) struct Replacement {
    pub start: usize,
    pub end: usize,
    pub new_value: String,
}

/// Applies multiple replacements to XML content while preserving formatting.
///
/// Replacements are sorted by start position and applied in order, with no overlaps allowed.
pub(crate) fn apply_replacements(xml: &str, mut replacements: Vec<Replacement>) -> String {
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
pub(crate) fn local_name(name: quick_xml::name::QName) -> String {
    String::from_utf8_lossy(name.local_name().as_ref()).to_string()
}

/// Creates a POM parse error.
pub(crate) fn parse_error(message: &str) -> anyhow::Error {
    DepupError::pom_parse_failed("POM XML", message).into()
}
