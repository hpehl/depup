//! Core types shared across all ecosystems: `Ecosystem`, `CheckerKind`, `CheckResult`, and `Outcome`.
//!
//! These types form the common currency passed between discovery, checking, filtering,
//! and output stages.

/// Supported dependency ecosystems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Ecosystem {
    Maven,
    Npm,
}

impl std::fmt::Display for Ecosystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Maven => write!(f, "Maven"),
            Self::Npm => write!(f, "npm"),
        }
    }
}

/// Classifies a check result for display grouping and styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CheckerKind {
    Dependency,
    Plugin,
    ToolVersion,
    NpmDep,
    NpmDevDep,
}

impl std::fmt::Display for CheckerKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dependency => write!(f, "Dependency"),
            Self::Plugin => write!(f, "Plugin"),
            Self::ToolVersion => write!(f, "Tool Version"),
            Self::NpmDep => write!(f, "Dependency"),
            Self::NpmDevDep => write!(f, "Dev Dependency"),
        }
    }
}

impl CheckerKind {
    /// Returns the terminal color style for this kind.
    pub fn color(self) -> console::Style {
        match self {
            Self::Dependency => console::Style::new().cyan(),
            Self::Plugin => console::Style::new().magenta(),
            Self::ToolVersion => console::Style::new().green(),
            Self::NpmDep | Self::NpmDevDep => console::Style::new().blue(),
        }
    }

    /// Returns the Unicode symbol used in the summary legend.
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Dependency => "\u{25cf}",
            Self::Plugin => "\u{25a0}",
            Self::ToolVersion => "\u{25b2}",
            Self::NpmDep | Self::NpmDevDep => "\u{25c6}",
        }
    }

    /// Returns the section header label used when grouping results by kind.
    pub fn group_label(self) -> &'static str {
        match self {
            Self::Dependency => "Dependencies",
            Self::Plugin => "Plugins",
            Self::ToolVersion => "Tool Versions",
            Self::NpmDep => "Dependencies",
            Self::NpmDevDep => "Dev Dependencies",
        }
    }
}

/// Result of checking a single dependency against its registry.
///
/// Constructed via the `checked()`, `skipped()`, or `error()` factory methods
/// to ensure all fields are set consistently. No post-construction mutation
/// except `with_version_property()`.
#[derive(Debug, Clone)]
pub struct CheckResult {
    pub ecosystem: Ecosystem,
    pub property_name: String,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub outdated: bool,
    pub skipped: bool,
    pub error: Option<String>,
    pub artifact: Option<String>,
    pub source: String,
    pub kind: CheckerKind,
    pub has_version_property: bool,
}

/// Derived state of a `CheckResult`, used by the output layer to choose formatting.
pub enum Outcome<'a> {
    UpToDate,
    Outdated { latest: &'a str },
    Skipped,
    Error { message: &'a str },
}

impl CheckResult {
    #[allow(clippy::too_many_arguments)]
    pub fn checked(
        ecosystem: Ecosystem,
        kind: CheckerKind,
        property_name: String,
        current_version: String,
        latest_version: String,
        is_outdated: bool,
        artifact: Option<String>,
        source: String,
    ) -> Self {
        Self {
            ecosystem,
            property_name,
            current_version,
            latest_version: Some(latest_version),
            outdated: is_outdated,
            skipped: false,
            error: None,
            artifact,
            source,
            kind,
            has_version_property: true,
        }
    }

    pub fn skipped(
        ecosystem: Ecosystem,
        kind: CheckerKind,
        property_name: String,
        current_version: String,
        artifact: Option<String>,
        source: String,
    ) -> Self {
        Self {
            ecosystem,
            property_name,
            current_version,
            latest_version: None,
            outdated: false,
            skipped: true,
            error: None,
            artifact,
            source,
            kind,
            has_version_property: true,
        }
    }

    pub fn error(
        ecosystem: Ecosystem,
        kind: CheckerKind,
        property_name: String,
        current_version: String,
        artifact: Option<String>,
        error: String,
        source: String,
    ) -> Self {
        Self {
            ecosystem,
            property_name,
            current_version,
            latest_version: None,
            outdated: false,
            skipped: false,
            error: Some(error),
            artifact,
            source,
            kind,
            has_version_property: true,
        }
    }

    /// Builder-style setter for whether this dependency has a managed version property (Maven only).
    pub fn with_version_property(mut self, has_version_property: bool) -> Self {
        self.has_version_property = has_version_property;
        self
    }

    /// Derives the display outcome from the result's state flags.
    pub fn outcome(&self) -> Outcome<'_> {
        if let Some(err) = &self.error {
            Outcome::Error { message: err }
        } else if self.skipped {
            Outcome::Skipped
        } else if self.outdated {
            Outcome::Outdated {
                latest: self.latest_version.as_deref().unwrap_or("?"),
            }
        } else {
            Outcome::UpToDate
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecosystem_display() {
        assert_eq!(Ecosystem::Maven.to_string(), "Maven");
        assert_eq!(Ecosystem::Npm.to_string(), "npm");
    }

    #[test]
    fn checked_result_fields() {
        let r = CheckResult::checked(
            Ecosystem::Maven,
            CheckerKind::Dependency,
            "version.junit".to_string(),
            "5.10.0".to_string(),
            "5.12.0".to_string(),
            true,
            Some("org.junit:junit".to_string()),
            String::new(),
        );
        assert!(r.outdated);
        assert!(!r.skipped);
        assert!(r.error.is_none());
        assert_eq!(r.latest_version.as_deref(), Some("5.12.0"));
    }

    #[test]
    fn skipped_result_fields() {
        let r = CheckResult::skipped(
            Ecosystem::Maven,
            CheckerKind::Dependency,
            "version.alpha".to_string(),
            "1.0.0-alpha".to_string(),
            None,
            String::new(),
        );
        assert!(r.skipped);
        assert!(!r.outdated);
        assert!(r.latest_version.is_none());
    }

    #[test]
    fn error_result_fields() {
        let r = CheckResult::error(
            Ecosystem::Maven,
            CheckerKind::Plugin,
            "version.plugin".to_string(),
            "1.0".to_string(),
            None,
            "timeout".to_string(),
            String::new(),
        );
        assert!(r.error.is_some());
        assert!(!r.outdated);
        assert!(!r.skipped);
    }

    #[test]
    fn outcome_matches_state() {
        let up_to_date = CheckResult::checked(
            Ecosystem::Maven,
            CheckerKind::Dependency,
            "p".into(),
            "1.0".into(),
            "1.0".into(),
            false,
            None,
            String::new(),
        );
        assert!(matches!(up_to_date.outcome(), Outcome::UpToDate));

        let outdated = CheckResult::checked(
            Ecosystem::Maven,
            CheckerKind::Dependency,
            "p".into(),
            "1.0".into(),
            "2.0".into(),
            true,
            None,
            String::new(),
        );
        assert!(matches!(outdated.outcome(), Outcome::Outdated { .. }));

        let skipped = CheckResult::skipped(
            Ecosystem::Maven,
            CheckerKind::Dependency,
            "p".into(),
            "1.0".into(),
            None,
            String::new(),
        );
        assert!(matches!(skipped.outcome(), Outcome::Skipped));

        let error = CheckResult::error(
            Ecosystem::Maven,
            CheckerKind::Dependency,
            "p".into(),
            "1.0".into(),
            None,
            "fail".into(),
            String::new(),
        );
        assert!(matches!(error.outcome(), Outcome::Error { .. }));
    }

    #[test]
    fn checker_kind_display() {
        assert_eq!(CheckerKind::Dependency.to_string(), "Dependency");
        assert_eq!(CheckerKind::Plugin.to_string(), "Plugin");
        assert_eq!(CheckerKind::ToolVersion.to_string(), "Tool Version");
        assert_eq!(CheckerKind::NpmDep.to_string(), "Dependency");
        assert_eq!(CheckerKind::NpmDevDep.to_string(), "Dev Dependency");
    }
}
