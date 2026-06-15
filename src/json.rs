use serde::Serialize;

use crate::registry::CheckResult;

#[derive(Debug, Serialize)]
pub struct JsonResult {
    pub ecosystem: String,
    pub property: String,
    pub current: String,
    pub latest: Option<String>,
    pub status: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact: Option<String>,
}

impl From<&CheckResult> for JsonResult {
    fn from(r: &CheckResult) -> Self {
        let status = if r.error.is_some() {
            "error"
        } else if r.skipped {
            "skipped"
        } else if r.outdated {
            "outdated"
        } else {
            "up-to-date"
        };
        Self {
            ecosystem: r.ecosystem.to_string().to_lowercase(),
            property: r.property_name.clone(),
            current: r.current_version.clone(),
            latest: r.latest_version.clone(),
            status: status.to_string(),
            kind: r.kind.to_string().to_lowercase(),
            error: r.error.clone(),
            artifact: r.artifact.clone(),
        }
    }
}
