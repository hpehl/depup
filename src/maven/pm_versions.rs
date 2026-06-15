use anyhow::Result;

use crate::constants::{self, NPM_REGISTRY_URL};
use crate::error::DepupError;
use crate::maven::discovery::VersionProperty;
use crate::registry::{CheckResult, CheckerKind, Ecosystem};
use crate::version;

const NPM_PACKAGES: &[(&str, &str)] = &[("npm", "npm"), ("pnpm", "pnpm"), ("yarn", "yarn")];

pub struct PmVersionsChecker {
    client: reqwest::Client,
    #[allow(dead_code)]
    releases_only: bool,
}

impl PmVersionsChecker {
    pub fn new(releases_only: bool) -> Self {
        Self {
            client: constants::http_client(),
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

    pub async fn check(
        &self,
        property: &VersionProperty,
        package: &str,
        source: &str,
    ) -> Result<CheckResult> {
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

        let prop_name = property.name.clone();
        let current = property.current_value.clone();
        let artifact = Some(package.to_string());
        let source = source.to_string();

        match latest {
            Some(latest) => {
                let is_outdated = version::is_newer(&current, &latest);
                Ok(CheckResult::checked(
                    Ecosystem::Maven,
                    CheckerKind::NpmPkg,
                    prop_name,
                    current,
                    latest,
                    is_outdated,
                    artifact,
                    source,
                ))
            }
            None => Ok(CheckResult::error(
                Ecosystem::Maven,
                CheckerKind::NpmPkg,
                prop_name,
                current,
                artifact,
                format!("No latest version found for {package}"),
                source,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_npm_packages() {
        assert_eq!(PmVersionsChecker::matches("version.npm"), Some("npm"));
        assert_eq!(PmVersionsChecker::matches("npm.version"), Some("npm"));
        assert_eq!(PmVersionsChecker::matches("version.pnpm"), Some("pnpm"));
        assert_eq!(PmVersionsChecker::matches("pnpm.version"), Some("pnpm"));
        assert_eq!(PmVersionsChecker::matches("version.yarn"), Some("yarn"));
        assert_eq!(PmVersionsChecker::matches("yarn.version"), Some("yarn"));
    }

    #[test]
    fn does_not_match_unrelated() {
        assert_eq!(PmVersionsChecker::matches("version.junit"), None);
        assert_eq!(PmVersionsChecker::matches("npm"), None);
        assert_eq!(PmVersionsChecker::matches("version.node"), None);
    }
}
