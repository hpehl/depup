//! Core types shared across all ecosystems.
//!
//! - [`Dependency`] identifies a dependency (ecosystem, kind, artifact, optional property, source).
//! - [`VersionStatus`] is an enum of mutually exclusive outcomes (up-to-date, outdated, skipped, error).
//! - [`VersionResult`] combines a `Dependency`, the current version, and a `VersionStatus`.
//!
//! These types form the common currency passed between discovery, checking, filtering,
//! and output stages.

pub mod audit;
pub mod check;
pub mod update;

pub use audit::{AuditResult, Severity, Vulnerability};
pub use check::{VersionResult, VersionStatus};
pub use update::{UpdateResult, UpdateStatus};

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
    NpmDep,
    NpmDevDep,
    Tool,
}

impl std::fmt::Display for DependencyKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dependency => write!(f, "Dependency"),
            Self::Plugin => write!(f, "Plugin"),
            Self::Tool => write!(f, "Tool"),
            Self::NpmDep => write!(f, "Dependency"),
            Self::NpmDevDep => write!(f, "Dev Dependency"),
        }
    }
}

/// Common accessors shared by all result types (`VersionResult`, `UpdateResult`, `AuditResult`).
///
/// Used by the output layer to group, sort, and format results generically
/// across subcommands.
pub trait CommandResult {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecosystem_display() {
        assert_eq!(Ecosystem::Maven.to_string(), "Maven");
        assert_eq!(Ecosystem::Npm.to_string(), "npm");
    }

    #[test]
    fn dependency_kind_display() {
        assert_eq!(DependencyKind::Dependency.to_string(), "Dependency");
        assert_eq!(DependencyKind::Plugin.to_string(), "Plugin");
        assert_eq!(DependencyKind::Tool.to_string(), "Tool");
        assert_eq!(DependencyKind::NpmDep.to_string(), "Dependency");
        assert_eq!(DependencyKind::NpmDevDep.to_string(), "Dev Dependency");
    }

    #[test]
    fn dependency_with_property() {
        let id = Dependency::new(
            Ecosystem::Maven,
            DependencyKind::Dependency,
            "org.junit:junit".to_string(),
            Some("version.junit".to_string()),
            String::new(),
        );
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
}
