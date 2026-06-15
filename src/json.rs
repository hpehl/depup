//! Serializable output types for `--json` mode.
//!
//! Converts internal `CheckResult` into a flat, JSON-friendly structure
//! suitable for machine consumption. Optional fields are omitted when empty/none.

use serde::Serialize;

use crate::registry::{CheckResult, CheckStatus, UpdateResult, UpdateStatus};

/// Flat JSON representation of a single dependency check result.
#[derive(Debug, Serialize)]
pub struct JsonResult {
    pub ecosystem: String,
    pub property: String,
    pub current: String,
    pub latest: Option<String>,
    pub status: String,
    pub kind: String,
    pub managed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact: Option<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub source: String,
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
            ecosystem: r.ecosystem().to_string().to_lowercase(),
            property: r.property_name().to_string(),
            current: r.current_version.clone(),
            latest: r.latest_version().map(ToString::to_string),
            status: status.to_string(),
            kind: r.kind().to_string().to_lowercase(),
            managed: r.has_version_property(),
            error,
            artifact: r.artifact().map(ToString::to_string),
            source: r.source().to_string(),
        }
    }
}

/// Flat JSON representation of a single dependency update result.
#[derive(Debug, Serialize)]
pub struct UpdateJsonResult {
    pub ecosystem: String,
    pub property: String,
    pub old_version: String,
    pub new_version: String,
    pub kind: String,
    pub managed: bool,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact: Option<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub source: String,
}

impl From<&UpdateResult> for UpdateJsonResult {
    fn from(r: &UpdateResult) -> Self {
        let (status, error) = match &r.status {
            UpdateStatus::Updated => ("updated", None),
            UpdateStatus::Error { message } => ("error", Some(message.clone())),
        };
        Self {
            ecosystem: r.ecosystem.to_string().to_lowercase(),
            property: r.property_name.clone(),
            old_version: r.old_version.clone(),
            new_version: r.new_version.clone(),
            kind: r.kind.to_string().to_lowercase(),
            managed: r.has_version_property,
            status: status.to_string(),
            error,
            artifact: r.artifact.clone(),
            source: r.source.clone(),
        }
    }
}

impl UpdateJsonResult {
    pub fn would_update(r: &CheckResult) -> Self {
        Self {
            ecosystem: r.ecosystem().to_string().to_lowercase(),
            property: r.property_name().to_string(),
            old_version: r.current_version.clone(),
            new_version: r.latest_version().unwrap_or("").to_string(),
            kind: r.kind().to_string().to_lowercase(),
            managed: r.has_version_property(),
            status: "would_update".to_string(),
            error: None,
            artifact: r.artifact().map(ToString::to_string),
            source: r.source().to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{CheckId, CheckerKind, Ecosystem};

    #[test]
    fn json_result_round_trip() {
        let r = CheckResult::checked(
            CheckId::new(
                Ecosystem::Maven,
                CheckerKind::Dependency,
                "version.junit".to_string(),
                Some("org.junit.jupiter:junit-jupiter".to_string()),
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
            CheckId::new(
                Ecosystem::Npm,
                CheckerKind::NpmDep,
                "react".to_string(),
                Some("react".to_string()),
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
            CheckId::new(
                Ecosystem::Maven,
                CheckerKind::Dependency,
                "version.junit".to_string(),
                Some("org.junit.jupiter:junit-jupiter".to_string()),
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
            CheckId::new(
                Ecosystem::Maven,
                CheckerKind::Plugin,
                "version.compiler".to_string(),
                Some("org.apache.maven.plugins:maven-compiler-plugin".to_string()),
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
            CheckId::new(
                Ecosystem::Maven,
                CheckerKind::Dependency,
                "p".to_string(),
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
        assert!(parsed.get("artifact").is_none());
    }
}
