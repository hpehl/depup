use super::Dependency;
use super::check::CheckResult;

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

#[cfg(test)]
impl Vulnerability {
    pub fn test(id: &str, severity: Severity) -> Self {
        Self {
            id: id.into(),
            aliases: Vec::new(),
            summary: String::new(),
            severity,
            url: None,
        }
    }
}

// ------------------------------------------------------ command result

/// Result of auditing a single dependency against OSV.
#[derive(Debug, Clone)]
pub struct AuditResult {
    pub dep: Dependency,
    pub version: String,
    pub vulnerabilities: Vec<Vulnerability>,
}

impl AuditResult {
    pub fn from_version_result(r: &CheckResult, vulnerabilities: Vec<Vulnerability>) -> Self {
        Self {
            dep: r.dep.clone(),
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

// ------------------------------------------------------ test

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::check::CheckResult;
    use crate::model::{CommandResult, Dependency, DependencyKind, Ecosystem};

    fn make_dep() -> Dependency {
        Dependency::new(
            Ecosystem::Maven,
            DependencyKind::Dependency,
            "org.example:lib".into(),
            Some("version.lib".into()),
            "pom.xml".into(),
        )
    }

    #[test]
    fn is_vulnerable_with_vulns() {
        let result = AuditResult {
            dep: make_dep(),
            version: "1.0.0".into(),
            vulnerabilities: vec![Vulnerability::test("CVE-1", Severity::High)],
        };
        assert!(result.is_vulnerable());
    }

    #[test]
    fn is_vulnerable_without_vulns() {
        let result = AuditResult {
            dep: make_dep(),
            version: "1.0.0".into(),
            vulnerabilities: Vec::new(),
        };
        assert!(!result.is_vulnerable());
    }

    #[test]
    fn max_severity_returns_highest() {
        let result = AuditResult {
            dep: make_dep(),
            version: "1.0.0".into(),
            vulnerabilities: vec![
                Vulnerability::test("V1", Severity::Low),
                Vulnerability::test("V2", Severity::Critical),
                Vulnerability::test("V3", Severity::Medium),
            ],
        };
        assert_eq!(result.max_severity(), Severity::Critical);
    }

    #[test]
    fn max_severity_empty_returns_unknown() {
        let result = AuditResult {
            dep: make_dep(),
            version: "1.0.0".into(),
            vulnerabilities: Vec::new(),
        };
        assert_eq!(result.max_severity(), Severity::Unknown);
    }

    #[test]
    fn from_version_result_copies_fields() {
        let check = CheckResult::checked(make_dep(), "1.0.0".into(), "2.0.0".into(), true);
        let vulns = vec![Vulnerability::test("CVE-1", Severity::High)];
        let audit = AuditResult::from_version_result(&check, vulns);
        assert_eq!(audit.version, "1.0.0");
        assert_eq!(audit.artifact(), "org.example:lib");
        assert_eq!(audit.property(), Some("version.lib"));
        assert_eq!(audit.source(), "pom.xml");
        assert_eq!(audit.vulnerabilities.len(), 1);
    }
}
