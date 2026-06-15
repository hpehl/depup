//! Package manager version checker for Maven POM tool-version properties.
//!
//! Checks properties like `version.npm`, `version.pnpm`, or `yarn.version`
//! against the npm registry's `dist-tags.latest` for the corresponding package.

use std::future::Future;
use std::pin::Pin;

use anyhow::Result;

use crate::constants::{self, NPM_REGISTRY_URL};
use crate::error::DepupError;
use crate::maven::discovery::VersionProperty;
use crate::maven::tool::ToolVersionChecker;
use crate::registry::{CheckResult, CheckerKind, Ecosystem};
use crate::version;

/// Generates the pattern-to-package mapping table and the flat pattern list.
macro_rules! pm_tools {
    ( $( ($pattern:expr, $package:expr) ),* $(,)? ) => {
        const PM_TOOLS: &[(&str, &str)] = &[ $( ($pattern, $package), )* ];
        const PM_PATTERN_NAMES: &[&str] = &[ $( $pattern, )* ];
    };
}

pm_tools![
    ("version.npm", "npm"),
    ("npm.version", "npm"),
    ("version.pnpm", "pnpm"),
    ("pnpm.version", "pnpm"),
    ("version.yarn", "yarn"),
    ("yarn.version", "yarn"),
];

/// Checks package manager version properties against the npm registry.
pub struct PmVersionsChecker {
    client: reqwest::Client,
}

impl PmVersionsChecker {
    pub fn new() -> Self {
        Self {
            client: constants::http_client(),
        }
    }

    /// Maps a property name to its npm package name (e.g., `version.pnpm` → `pnpm`).
    fn resolve_package(property_name: &str) -> Option<&'static str> {
        PM_TOOLS
            .iter()
            .find(|(pattern, _)| *pattern == property_name)
            .map(|(_, package)| *package)
    }

    async fn fetch_and_check(
        &self,
        property: &VersionProperty,
        source: &str,
    ) -> Result<CheckResult> {
        let Some(package) = Self::resolve_package(&property.name) else {
            return Ok(CheckResult::error(
                Ecosystem::Maven,
                CheckerKind::ToolVersion,
                property.name.clone(),
                property.current_value.clone(),
                None,
                format!("Unknown tool property: {}", property.name),
                source.to_string(),
            ));
        };

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
                    CheckerKind::ToolVersion,
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
                CheckerKind::ToolVersion,
                prop_name,
                current,
                artifact,
                format!("No latest version found for {package}"),
                source,
            )),
        }
    }
}

impl ToolVersionChecker for PmVersionsChecker {
    fn patterns(&self) -> &[&str] {
        PM_PATTERN_NAMES
    }

    fn label(&self, property: &VersionProperty) -> String {
        Self::resolve_package(&property.name)
            .unwrap_or("unknown")
            .to_string()
    }

    fn check<'a>(
        &'a self,
        property: &'a VersionProperty,
        source: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<CheckResult>> + Send + 'a>> {
        Box::pin(self.fetch_and_check(property, source))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_match_pm_tools() {
        let checker = PmVersionsChecker::new();
        let patterns = checker.patterns();
        assert!(patterns.contains(&"version.npm"));
        assert!(patterns.contains(&"npm.version"));
        assert!(patterns.contains(&"version.pnpm"));
        assert!(patterns.contains(&"pnpm.version"));
        assert!(patterns.contains(&"version.yarn"));
        assert!(patterns.contains(&"yarn.version"));
    }

    #[test]
    fn resolve_package_names() {
        assert_eq!(PmVersionsChecker::resolve_package("version.npm"), Some("npm"));
        assert_eq!(PmVersionsChecker::resolve_package("npm.version"), Some("npm"));
        assert_eq!(PmVersionsChecker::resolve_package("version.pnpm"), Some("pnpm"));
        assert_eq!(PmVersionsChecker::resolve_package("pnpm.version"), Some("pnpm"));
        assert_eq!(PmVersionsChecker::resolve_package("version.yarn"), Some("yarn"));
        assert_eq!(PmVersionsChecker::resolve_package("yarn.version"), Some("yarn"));
    }

    #[test]
    fn does_not_match_unrelated() {
        assert_eq!(PmVersionsChecker::resolve_package("version.junit"), None);
        assert_eq!(PmVersionsChecker::resolve_package("npm"), None);
        assert_eq!(PmVersionsChecker::resolve_package("version.node"), None);
    }

    #[test]
    fn label_resolves_from_property() {
        let checker = PmVersionsChecker::new();
        let prop = VersionProperty {
            name: "version.pnpm".to_string(),
            current_value: "9.0.0".to_string(),
        };
        assert_eq!(checker.label(&prop), "pnpm");
    }
}
