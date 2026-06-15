//! Core types shared across all ecosystems.
//!
//! - [`CheckId`] groups the identity of a checked dependency (ecosystem, kind, name, source).
//! - [`CheckStatus`] is an enum of mutually exclusive outcomes (up-to-date, outdated, skipped, error).
//! - [`CheckResult`] combines a `CheckId`, the current version, and a `CheckStatus`.
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

/// Identity of a checked dependency: what was checked, where it came from.
#[derive(Debug, Clone)]
pub struct CheckId {
    pub ecosystem: Ecosystem,
    pub kind: CheckerKind,
    pub property_name: String,
    pub artifact: Option<String>,
    pub source: String,
    pub has_version_property: bool,
}

impl CheckId {
    pub fn new(
        ecosystem: Ecosystem,
        kind: CheckerKind,
        property_name: String,
        artifact: Option<String>,
        source: String,
    ) -> Self {
        Self {
            ecosystem,
            kind,
            property_name,
            artifact,
            source,
            has_version_property: true,
        }
    }

    /// Sets whether this dependency has a managed version property (Maven only).
    pub fn with_version_property(mut self, has_version_property: bool) -> Self {
        self.has_version_property = has_version_property;
        self
    }
}

/// The outcome of checking a dependency version.
#[derive(Debug, Clone)]
pub enum CheckStatus {
    UpToDate { latest: String },
    Outdated { latest: String },
    Skipped,
    Error { message: String },
}

/// Result of checking a single dependency against its registry.
#[derive(Debug, Clone)]
pub struct CheckResult {
    pub id: CheckId,
    pub current_version: String,
    pub status: CheckStatus,
}

impl CheckResult {
    pub fn checked(id: CheckId, current: String, latest: String, is_outdated: bool) -> Self {
        let status = if is_outdated {
            CheckStatus::Outdated { latest }
        } else {
            CheckStatus::UpToDate { latest }
        };
        Self {
            id,
            current_version: current,
            status,
        }
    }

    pub fn skipped(id: CheckId, current: String) -> Self {
        Self {
            id,
            current_version: current,
            status: CheckStatus::Skipped,
        }
    }

    pub fn error(id: CheckId, current: String, message: String) -> Self {
        Self {
            id,
            current_version: current,
            status: CheckStatus::Error { message },
        }
    }

    pub fn ecosystem(&self) -> Ecosystem {
        self.id.ecosystem
    }

    pub fn kind(&self) -> CheckerKind {
        self.id.kind
    }

    pub fn property_name(&self) -> &str {
        &self.id.property_name
    }

    pub fn artifact(&self) -> Option<&str> {
        self.id.artifact.as_deref()
    }

    pub fn source(&self) -> &str {
        &self.id.source
    }

    pub fn has_version_property(&self) -> bool {
        self.id.has_version_property
    }

    pub fn is_outdated(&self) -> bool {
        matches!(self.status, CheckStatus::Outdated { .. })
    }

    pub fn is_skipped(&self) -> bool {
        matches!(self.status, CheckStatus::Skipped)
    }

    pub fn error_message(&self) -> Option<&str> {
        match &self.status {
            CheckStatus::Error { message } => Some(message),
            _ => None,
        }
    }

    pub fn latest_version(&self) -> Option<&str> {
        match &self.status {
            CheckStatus::UpToDate { latest } | CheckStatus::Outdated { latest } => Some(latest),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dep_id() -> CheckId {
        CheckId::new(
            Ecosystem::Maven,
            CheckerKind::Dependency,
            "version.junit".to_string(),
            Some("org.junit:junit".to_string()),
            String::new(),
        )
    }

    #[test]
    fn ecosystem_display() {
        assert_eq!(Ecosystem::Maven.to_string(), "Maven");
        assert_eq!(Ecosystem::Npm.to_string(), "npm");
    }

    #[test]
    fn checked_result_fields() {
        let r = CheckResult::checked(
            dep_id(),
            "5.10.0".to_string(),
            "5.12.0".to_string(),
            true,
        );
        assert!(r.is_outdated());
        assert!(!r.is_skipped());
        assert!(r.error_message().is_none());
        assert_eq!(r.latest_version(), Some("5.12.0"));
    }

    #[test]
    fn skipped_result_fields() {
        let id = CheckId::new(
            Ecosystem::Maven,
            CheckerKind::Dependency,
            "version.alpha".to_string(),
            None,
            String::new(),
        );
        let r = CheckResult::skipped(id, "1.0.0-alpha".to_string());
        assert!(r.is_skipped());
        assert!(!r.is_outdated());
        assert!(r.latest_version().is_none());
    }

    #[test]
    fn error_result_fields() {
        let id = CheckId::new(
            Ecosystem::Maven,
            CheckerKind::Plugin,
            "version.plugin".to_string(),
            None,
            String::new(),
        );
        let r = CheckResult::error(id, "1.0".to_string(), "timeout".to_string());
        assert!(r.error_message().is_some());
        assert!(!r.is_outdated());
        assert!(!r.is_skipped());
    }

    #[test]
    fn status_matches_state() {
        let up_to_date = CheckResult::checked(
            CheckId::new(Ecosystem::Maven, CheckerKind::Dependency, "p".into(), None, String::new()),
            "1.0".into(),
            "1.0".into(),
            false,
        );
        assert!(matches!(up_to_date.status, CheckStatus::UpToDate { .. }));

        let outdated = CheckResult::checked(
            CheckId::new(Ecosystem::Maven, CheckerKind::Dependency, "p".into(), None, String::new()),
            "1.0".into(),
            "2.0".into(),
            true,
        );
        assert!(matches!(outdated.status, CheckStatus::Outdated { .. }));

        let skipped = CheckResult::skipped(
            CheckId::new(Ecosystem::Maven, CheckerKind::Dependency, "p".into(), None, String::new()),
            "1.0".into(),
        );
        assert!(matches!(skipped.status, CheckStatus::Skipped));

        let error = CheckResult::error(
            CheckId::new(Ecosystem::Maven, CheckerKind::Dependency, "p".into(), None, String::new()),
            "1.0".into(),
            "fail".into(),
        );
        assert!(matches!(error.status, CheckStatus::Error { .. }));
    }

    #[test]
    fn checker_kind_display() {
        assert_eq!(CheckerKind::Dependency.to_string(), "Dependency");
        assert_eq!(CheckerKind::Plugin.to_string(), "Plugin");
        assert_eq!(CheckerKind::ToolVersion.to_string(), "Tool Version");
        assert_eq!(CheckerKind::NpmDep.to_string(), "Dependency");
        assert_eq!(CheckerKind::NpmDevDep.to_string(), "Dev Dependency");
    }

    #[test]
    fn check_id_defaults_version_property_true() {
        let id = dep_id();
        assert!(id.has_version_property);
    }

    #[test]
    fn check_id_with_version_property_false() {
        let id = dep_id().with_version_property(false);
        assert!(!id.has_version_property);
    }
}
