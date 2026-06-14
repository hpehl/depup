use anyhow::Result;
use std::time::Duration;

use crate::constants::{HTTP_TIMEOUT_SECS, NPM_REGISTRY_URL};
use crate::error::DepupError;
use crate::maven::discovery::VersionProperty;
use crate::registry::{CheckResult, CheckerKind, Ecosystem};
use crate::version;

const NPM_PACKAGES: &[(&str, &str)] = &[("npm", "npm"), ("pnpm", "pnpm"), ("yarn", "yarn")];

pub struct NpmChecker {
    client: reqwest::Client,
    #[allow(dead_code)]
    releases_only: bool,
}

impl NpmChecker {
    pub fn new(releases_only: bool) -> Self {
        let client = reqwest::Client::builder()
            .user_agent(format!("depup/{}", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .build()
            .expect("Failed to create HTTP client");
        Self {
            client,
            releases_only,
        }
    }

    pub fn matches(property_name: &str) -> Option<&'static str> {
        for &(key, package) in NPM_PACKAGES {
            if property_name == format!("version.{key}")
                || property_name == format!("{key}.version")
            {
                return Some(package);
            }
        }
        None
    }

    pub async fn check(&self, property: &VersionProperty, package: &str) -> Result<CheckResult> {
        let url = format!("{NPM_REGISTRY_URL}/{package}");

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| DepupError::http_request_failed(&url, &e.to_string()))?;

        if !resp.status().is_success() {
            return Err(
                DepupError::http_request_failed(&url, &format!("HTTP {}", resp.status())).into(),
            );
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| DepupError::http_request_failed(&url, &e.to_string()))?;

        let latest = body["dist-tags"]["latest"]
            .as_str()
            .map(ToString::to_string);

        match latest {
            Some(latest) => Ok(CheckResult {
                ecosystem: Ecosystem::Maven,
                property_name: property.name.clone(),
                current_version: property.current_value.clone(),
                latest_version: Some(latest.clone()),
                outdated: version::is_newer(&property.current_value, &latest),
                skipped: false,
                error: None,
                artifact: Some(package.to_string()),
                kind: CheckerKind::Npm,
            }),
            None => Ok(CheckResult {
                ecosystem: Ecosystem::Maven,
                property_name: property.name.clone(),
                current_version: property.current_value.clone(),
                latest_version: None,
                outdated: false,
                skipped: false,
                error: Some(format!("No latest version found for {package}")),
                artifact: Some(package.to_string()),
                kind: CheckerKind::Npm,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_npm_packages() {
        assert_eq!(NpmChecker::matches("version.npm"), Some("npm"));
        assert_eq!(NpmChecker::matches("npm.version"), Some("npm"));
        assert_eq!(NpmChecker::matches("version.pnpm"), Some("pnpm"));
        assert_eq!(NpmChecker::matches("pnpm.version"), Some("pnpm"));
        assert_eq!(NpmChecker::matches("version.yarn"), Some("yarn"));
        assert_eq!(NpmChecker::matches("yarn.version"), Some("yarn"));
    }

    #[test]
    fn does_not_match_unrelated() {
        assert_eq!(NpmChecker::matches("version.junit"), None);
        assert_eq!(NpmChecker::matches("npm"), None);
        assert_eq!(NpmChecker::matches("version.node"), None);
    }
}
