//! Core types shared across all ecosystems.
//!
//! - [`Dependency`] identifies a dependency (ecosystem, kind, artifact, optional property, source).
//! - [`VersionStatus`] is an enum of mutually exclusive outcomes (up-to-date, outdated, skipped, error).
//! - [`VersionResult`] combines a `Dependency`, the current version, and a `VersionStatus`.
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

/// Classifies a dependency for display grouping and styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DependencyKind {
    Dependency,
    Plugin,
    ToolVersion,
    NpmDep,
    NpmDevDep,
}

impl std::fmt::Display for DependencyKind {
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

/// Common accessors shared by all result types (`VersionResult`, `UpdateResult`, `AuditResult`).
///
/// Used by the output layer to group, sort, and format results generically
/// across subcommands.
pub trait DependencyInfo {
    fn ecosystem(&self) -> Ecosystem;
    fn kind(&self) -> DependencyKind;
    fn artifact(&self) -> &str;
    fn property(&self) -> Option<&str>;
    fn source(&self) -> &str;

    fn has_property(&self) -> bool {
        self.property().is_some()
    }
}

/// Identity of a dependency: what it is and where it came from.
#[derive(Debug, Clone)]
pub struct Dependency {
    pub ecosystem: Ecosystem,
    pub kind: DependencyKind,
    /// Display name / coordinates: Maven `"groupId:artifactId"`, npm package name, or tool label.
    pub artifact: String,
    /// Maven version property name (e.g. `"version.junit"`). `None` for inline Maven versions,
    /// npm packages, and tool versions.
    pub property: Option<String>,
    /// Relative path to the file this dependency was found in.
    pub source: String,
}

impl Dependency {
    pub fn new(
        ecosystem: Ecosystem,
        kind: DependencyKind,
        artifact: String,
        property: Option<String>,
        source: String,
    ) -> Self {
        Self {
            ecosystem,
            kind,
            artifact,
            property,
            source,
        }
    }
}

/// The outcome of checking a dependency version.
#[derive(Debug, Clone)]
pub enum VersionStatus {
    UpToDate { latest: String },
    Outdated { latest: String },
    Skipped,
    Error { message: String },
}

/// Result of checking a single dependency against its registry.
#[derive(Debug, Clone)]
pub struct VersionResult {
    pub id: Dependency,
    pub current_version: String,
    pub status: VersionStatus,
}

impl DependencyInfo for VersionResult {
    fn ecosystem(&self) -> Ecosystem {
        self.id.ecosystem
    }
    fn kind(&self) -> DependencyKind {
        self.id.kind
    }
    fn artifact(&self) -> &str {
        &self.id.artifact
    }
    fn property(&self) -> Option<&str> {
        self.id.property.as_deref()
    }
    fn source(&self) -> &str {
        &self.id.source
    }
}

impl VersionResult {
    pub fn checked(id: Dependency, current: String, latest: String, is_outdated: bool) -> Self {
        let status = if is_outdated {
            VersionStatus::Outdated { latest }
        } else {
            VersionStatus::UpToDate { latest }
        };
        Self {
            id,
            current_version: current,
            status,
        }
    }

    pub fn skipped(id: Dependency, current: String) -> Self {
        Self {
            id,
            current_version: current,
            status: VersionStatus::Skipped,
        }
    }

    pub fn error(id: Dependency, current: String, message: String) -> Self {
        Self {
            id,
            current_version: current,
            status: VersionStatus::Error { message },
        }
    }

    pub fn is_outdated(&self) -> bool {
        matches!(self.status, VersionStatus::Outdated { .. })
    }

    pub fn is_skipped(&self) -> bool {
        matches!(self.status, VersionStatus::Skipped)
    }

    pub fn error_message(&self) -> Option<&str> {
        match &self.status {
            VersionStatus::Error { message } => Some(message),
            _ => None,
        }
    }

    pub fn latest_version(&self) -> Option<&str> {
        match &self.status {
            VersionStatus::UpToDate { latest } | VersionStatus::Outdated { latest } => Some(latest),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Audit types
// ---------------------------------------------------------------------------

/// CVSS-based severity level for vulnerabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Unknown,
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    /// Parses from a CVSS score (0.0–10.0).
    pub fn from_cvss(score: f64) -> Self {
        if score >= 9.0 {
            Self::Critical
        } else if score >= 7.0 {
            Self::High
        } else if score >= 4.0 {
            Self::Medium
        } else if score > 0.0 {
            Self::Low
        } else {
            Self::Unknown
        }
    }

    /// Parses from a severity string (case-insensitive).
    pub fn from_str_label(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "CRITICAL" => Self::Critical,
            "HIGH" => Self::High,
            "MEDIUM" | "MODERATE" => Self::Medium,
            "LOW" => Self::Low,
            _ => Self::Unknown,
        }
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Critical => write!(f, "CRITICAL"),
            Self::High => write!(f, "HIGH"),
            Self::Medium => write!(f, "MEDIUM"),
            Self::Low => write!(f, "LOW"),
            Self::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

/// A single known vulnerability for a dependency.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Vulnerability {
    pub id: String,
    pub aliases: Vec<String>,
    pub summary: String,
    pub severity: Severity,
    pub url: Option<String>,
}

/// Result of auditing a single dependency against OSV.
#[derive(Debug, Clone)]
pub struct AuditResult {
    pub id: Dependency,
    pub version: String,
    pub vulnerabilities: Vec<Vulnerability>,
}

impl DependencyInfo for AuditResult {
    fn ecosystem(&self) -> Ecosystem {
        self.id.ecosystem
    }
    fn kind(&self) -> DependencyKind {
        self.id.kind
    }
    fn artifact(&self) -> &str {
        &self.id.artifact
    }
    fn property(&self) -> Option<&str> {
        self.id.property.as_deref()
    }
    fn source(&self) -> &str {
        &self.id.source
    }
}

impl AuditResult {
    pub fn from_version_result(r: &VersionResult, vulnerabilities: Vec<Vulnerability>) -> Self {
        Self {
            id: r.id.clone(),
            version: r.current_version.clone(),
            vulnerabilities,
        }
    }

    pub fn max_severity(&self) -> Severity {
        self.vulnerabilities
            .iter()
            .map(|v| v.severity)
            .max()
            .unwrap_or(Severity::Unknown)
    }

    pub fn is_vulnerable(&self) -> bool {
        !self.vulnerabilities.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Update types
// ---------------------------------------------------------------------------

/// The outcome of updating a dependency version.
#[derive(Debug, Clone)]
pub enum UpdateStatus {
    Updated,
    Error { message: String },
}

/// Result of updating a single dependency.
#[derive(Debug, Clone)]
pub struct UpdateResult {
    pub id: Dependency,
    pub old_version: String,
    pub new_version: String,
    pub status: UpdateStatus,
}

impl DependencyInfo for UpdateResult {
    fn ecosystem(&self) -> Ecosystem {
        self.id.ecosystem
    }
    fn kind(&self) -> DependencyKind {
        self.id.kind
    }
    fn artifact(&self) -> &str {
        &self.id.artifact
    }
    fn property(&self) -> Option<&str> {
        self.id.property.as_deref()
    }
    fn source(&self) -> &str {
        &self.id.source
    }
}

impl UpdateResult {
    pub fn updated(check: &VersionResult, new_version: String) -> Self {
        Self {
            id: check.id.clone(),
            old_version: check.current_version.clone(),
            new_version,
            status: UpdateStatus::Updated,
        }
    }

    pub fn error(check: &VersionResult, message: String) -> Self {
        let new_version = check.latest_version().unwrap_or("").to_string();
        Self {
            id: check.id.clone(),
            old_version: check.current_version.clone(),
            new_version,
            status: UpdateStatus::Error { message },
        }
    }

    pub fn is_error(&self) -> bool {
        matches!(self.status, UpdateStatus::Error { .. })
    }

    #[cfg(test)]
    pub fn error_message(&self) -> Option<&str> {
        match &self.status {
            UpdateStatus::Error { message } => Some(message),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dep_id() -> Dependency {
        Dependency::new(
            Ecosystem::Maven,
            DependencyKind::Dependency,
            "org.junit:junit".to_string(),
            Some("version.junit".to_string()),
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
        let r = VersionResult::checked(dep_id(), "5.10.0".to_string(), "5.12.0".to_string(), true);
        assert!(r.is_outdated());
        assert!(!r.is_skipped());
        assert!(r.error_message().is_none());
        assert_eq!(r.latest_version(), Some("5.12.0"));
    }

    #[test]
    fn skipped_result_fields() {
        let id = Dependency::new(
            Ecosystem::Maven,
            DependencyKind::Dependency,
            "org.alpha:alpha".to_string(),
            Some("version.alpha".to_string()),
            String::new(),
        );
        let r = VersionResult::skipped(id, "1.0.0-alpha".to_string());
        assert!(r.is_skipped());
        assert!(!r.is_outdated());
        assert!(r.latest_version().is_none());
    }

    #[test]
    fn error_result_fields() {
        let id = Dependency::new(
            Ecosystem::Maven,
            DependencyKind::Plugin,
            "org.plugin:plugin".to_string(),
            Some("version.plugin".to_string()),
            String::new(),
        );
        let r = VersionResult::error(id, "1.0".to_string(), "timeout".to_string());
        assert!(r.error_message().is_some());
        assert!(!r.is_outdated());
        assert!(!r.is_skipped());
    }

    #[test]
    fn status_matches_state() {
        let up_to_date = VersionResult::checked(
            Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                "g:a".into(),
                None,
                String::new(),
            ),
            "1.0".into(),
            "1.0".into(),
            false,
        );
        assert!(matches!(up_to_date.status, VersionStatus::UpToDate { .. }));

        let outdated = VersionResult::checked(
            Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                "g:a".into(),
                None,
                String::new(),
            ),
            "1.0".into(),
            "2.0".into(),
            true,
        );
        assert!(matches!(outdated.status, VersionStatus::Outdated { .. }));

        let skipped = VersionResult::skipped(
            Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                "g:a".into(),
                None,
                String::new(),
            ),
            "1.0".into(),
        );
        assert!(matches!(skipped.status, VersionStatus::Skipped));

        let error = VersionResult::error(
            Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                "g:a".into(),
                None,
                String::new(),
            ),
            "1.0".into(),
            "fail".into(),
        );
        assert!(matches!(error.status, VersionStatus::Error { .. }));
    }

    #[test]
    fn dependency_kind_display() {
        assert_eq!(DependencyKind::Dependency.to_string(), "Dependency");
        assert_eq!(DependencyKind::Plugin.to_string(), "Plugin");
        assert_eq!(DependencyKind::ToolVersion.to_string(), "Tool Version");
        assert_eq!(DependencyKind::NpmDep.to_string(), "Dependency");
        assert_eq!(DependencyKind::NpmDevDep.to_string(), "Dev Dependency");
    }

    #[test]
    fn dependency_with_property() {
        let id = dep_id();
        assert!(id.property.is_some());
        assert_eq!(id.property, Some("version.junit".to_string()));
    }

    #[test]
    fn dependency_without_property() {
        let id = Dependency::new(
            Ecosystem::Maven,
            DependencyKind::Dependency,
            "com.google.guava:guava".into(),
            None,
            String::new(),
        );
        assert!(id.property.is_none());
    }

    #[test]
    fn update_result_from_check_result() {
        let check = VersionResult::checked(dep_id(), "5.10.0".into(), "5.12.0".into(), true);
        let update = UpdateResult::updated(&check, "5.12.0".into());
        assert_eq!(update.id.ecosystem, Ecosystem::Maven);
        assert_eq!(update.old_version, "5.10.0");
        assert_eq!(update.new_version, "5.12.0");
        assert!(!update.is_error());
    }

    #[test]
    fn update_result_error() {
        let check = VersionResult::checked(dep_id(), "5.10.0".into(), "5.12.0".into(), true);
        let update = UpdateResult::error(&check, "write failed".into());
        assert!(update.is_error());
        assert_eq!(update.error_message(), Some("write failed"));
    }
}
