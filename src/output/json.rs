//! Serializable output types for `--json` mode.
//!
//! Converts internal `VersionResult` into a flat, JSON-friendly structure
//! suitable for machine consumption. Optional fields are omitted when empty/none.

use serde::Serialize;

use crate::model::{
    AuditResult, CheckResult, CheckStatus, CommandResult, UpdateResult, UpdateStatus, Vulnerability,
};

/// Common fields shared by all JSON output types.
#[derive(Debug, Serialize)]
struct JsonBase {
    ecosystem: String,
    artifact: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    property: Option<String>,
    kind: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    source: String,
}

impl JsonBase {
    fn from_result(r: &impl CommandResult) -> Self {
        Self {
            ecosystem: r.ecosystem().to_string().to_lowercase(),
            artifact: r.artifact().to_string(),
            property: r.property().map(ToString::to_string),
            kind: r.kind().to_string().to_lowercase(),
            source: r.source().to_string(),
        }
    }
}

/// Flat JSON representation of a single dependency check result.
#[derive(Debug, Serialize)]
pub struct JsonResult {
    #[serde(flatten)]
    base: JsonBase,
    pub current: String,
    pub latest: Option<String>,
    pub status: String,
    pub managed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl From<&CheckResult> for JsonResult {
    fn from(r: &CheckResult) -> Self {
        let (status, error) = match &r.status {
            CheckStatus::UpToDate { .. } => ("up-to-date", None),
            CheckStatus::Outdated { .. } => ("outdated", None),
            CheckStatus::Skipped => ("skipped", None),
            CheckStatus::Error { message } => ("error", Some(message.clone())),
        };
        Self {
            base: JsonBase::from_result(r),
            current: r.current_version.clone(),
            latest: r.latest_version().map(ToString::to_string),
            status: status.to_string(),
            managed: r.has_property(),
            error,
        }
    }
}

/// Flat JSON representation of a single dependency update result.
#[derive(Debug, Serialize)]
pub struct UpdateJsonResult {
    #[serde(flatten)]
    base: JsonBase,
    pub old_version: String,
    pub new_version: String,
    pub managed: bool,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl From<&UpdateResult> for UpdateJsonResult {
    fn from(r: &UpdateResult) -> Self {
        let (status, error) = match &r.status {
            UpdateStatus::Updated => ("updated", None),
            UpdateStatus::Error { message } => ("error", Some(message.clone())),
        };
        Self {
            base: JsonBase::from_result(r),
            old_version: r.old_version.clone(),
            new_version: r.new_version.clone(),
            managed: r.has_property(),
            status: status.to_string(),
            error,
        }
    }
}

/// Flat JSON representation of a single dependency audit result.
#[derive(Debug, Serialize)]
pub struct AuditJsonResult {
    #[serde(flatten)]
    base: JsonBase,
    pub version: String,
    pub vulnerable: bool,
    pub vulnerabilities: Vec<VulnerabilityJson>,
}

impl From<&AuditResult> for AuditJsonResult {
    fn from(r: &AuditResult) -> Self {
        Self {
            base: JsonBase::from_result(r),
            version: r.version.clone(),
            vulnerable: r.is_vulnerable(),
            vulnerabilities: r
                .vulnerabilities
                .iter()
                .map(VulnerabilityJson::from)
                .collect(),
        }
    }
}

/// JSON representation of a single vulnerability.
#[derive(Debug, Serialize)]
pub struct VulnerabilityJson {
    pub id: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub summary: String,
    pub severity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl From<&Vulnerability> for VulnerabilityJson {
    fn from(v: &Vulnerability) -> Self {
        Self {
            id: v.id.clone(),
            aliases: v.aliases.clone(),
            summary: v.summary.clone(),
            severity: v.severity.to_string().to_lowercase(),
            url: v.url.clone(),
        }
    }
}

impl UpdateJsonResult {
    pub fn would_update(r: &CheckResult) -> Self {
        Self {
            base: JsonBase::from_result(r),
            old_version: r.current_version.clone(),
            new_version: r.latest_version().unwrap_or("").to_string(),
            managed: r.has_property(),
            status: "would_update".to_string(),
            error: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Dependency, DependencyKind, Ecosystem};

    #[test]
    fn json_result_round_trip() {
        let r = CheckResult::checked(
            Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                "org.junit.jupiter:junit-jupiter".to_string(),
                Some("version.junit".to_string()),
                "pom.xml".to_string(),
            ),
            "5.10.0".to_string(),
            "5.12.0".to_string(),
            true,
        );
        let json_result = JsonResult::from(&r);
        let serialized = serde_json::to_string(&json_result).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();

        assert_eq!(parsed["ecosystem"], "maven");
        assert_eq!(parsed["property"], "version.junit");
        assert_eq!(parsed["current"], "5.10.0");
        assert_eq!(parsed["latest"], "5.12.0");
        assert_eq!(parsed["status"], "outdated");
        assert_eq!(parsed["kind"], "dependency");
        assert_eq!(parsed["artifact"], "org.junit.jupiter:junit-jupiter");
        assert!(parsed.get("error").is_none());
    }

    #[test]
    fn json_result_error_includes_error_field() {
        let r = CheckResult::error(
            Dependency::new(
                Ecosystem::Npm,
                DependencyKind::NpmDep,
                "react".to_string(),
                None,
                String::new(),
            ),
            "18.0.0".to_string(),
            "not found".to_string(),
        );
        let json_result = JsonResult::from(&r);
        let serialized = serde_json::to_string(&json_result).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();

        assert_eq!(parsed["status"], "error");
        assert_eq!(parsed["error"], "not found");
    }

    #[test]
    fn update_json_result_updated() {
        let check = CheckResult::checked(
            Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                "org.junit.jupiter:junit-jupiter".to_string(),
                Some("version.junit".to_string()),
                "pom.xml".to_string(),
            ),
            "5.10.0".to_string(),
            "5.12.0".to_string(),
            true,
        );
        let update = UpdateResult::updated(&check, "5.12.0".to_string());
        let json = UpdateJsonResult::from(&update);
        let serialized = serde_json::to_string(&json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(parsed["status"], "updated");
        assert_eq!(parsed["old_version"], "5.10.0");
        assert_eq!(parsed["new_version"], "5.12.0");
        assert_eq!(parsed["ecosystem"], "maven");
        assert_eq!(parsed["kind"], "dependency");
        assert!(parsed.get("error").is_none());
    }

    #[test]
    fn update_json_result_would_update() {
        let check = CheckResult::checked(
            Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Plugin,
                "org.apache.maven.plugins:maven-compiler-plugin".to_string(),
                Some("version.compiler".to_string()),
                "pom.xml".to_string(),
            ),
            "3.11.0".to_string(),
            "3.13.0".to_string(),
            true,
        );
        let json = UpdateJsonResult::would_update(&check);
        let serialized = serde_json::to_string(&json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(parsed["status"], "would_update");
        assert_eq!(parsed["kind"], "plugin");
    }

    #[test]
    fn json_result_skips_none_fields() {
        let r = CheckResult::checked(
            Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                "g:a".to_string(),
                None,
                String::new(),
            ),
            "1.0".to_string(),
            "1.0".to_string(),
            false,
        );
        let json_result = JsonResult::from(&r);
        let serialized = serde_json::to_string(&json_result).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();

        assert!(parsed.get("error").is_none());
        assert!(parsed.get("property").is_none());
        assert_eq!(parsed["artifact"], "g:a");
    }
}
