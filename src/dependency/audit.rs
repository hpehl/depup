use super::check::VersionResult;
use super::{Dependency, DependencyInfo, DependencyKind, Ecosystem};

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
    /// Parses from a CVSS score (0.0-10.0).
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
