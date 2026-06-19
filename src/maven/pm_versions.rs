//! Package manager version resolver for Maven POM tool-version properties.
//!
//! Resolves properties like `version.npm`, `version.pnpm`, or `yarn.version`
//! against the npm registry's `dist-tags.latest` for the corresponding package.

use std::future::Future;
use std::pin::Pin;

use anyhow::Result;

use crate::constants::{self, NPM_REGISTRY_URL};
use crate::maven::discovery::VersionProperty;
use crate::maven::tool::ToolVersionResolver;
use crate::model::{CheckResult, Dependency, DependencyKind, Ecosystem};
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

/// Resolves package manager version properties against the npm registry.
pub struct PmVersionsResolver;

impl PmVersionsResolver {
    pub fn new() -> Self {
        Self
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
            let id = Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Tool,
                property.name.clone(),
                None,
                source.to_string(),
            );
            return Ok(CheckResult::error(
                id,
                property.current_value.clone(),
                format!("Unknown tool property: {}", property.name),
            ));
        };

        let id = Dependency::new(
            Ecosystem::Maven,
            DependencyKind::Tool,
            package.to_string(),
            None,
            source.to_string(),
        );
        let current = property.current_value.clone();

        let url = format!("{NPM_REGISTRY_URL}/{package}");
        let body: serde_json::Value = constants::fetch_json(&url).await?;

        let latest = body["dist-tags"]["latest"]
            .as_str()
            .map(ToString::to_string);

        match latest {
            Some(latest) => {
                let is_outdated = version::is_newer(&current, &latest);
                Ok(CheckResult::checked(id, current, latest, is_outdated))
            }
            None => Ok(CheckResult::error(
                id,
                current,
                format!("No latest version found for {package}"),
            )),
        }
    }
}

impl ToolVersionResolver for PmVersionsResolver {
    fn patterns(&self) -> &[&str] {
        PM_PATTERN_NAMES
    }

    fn label(&self, property: &VersionProperty) -> String {
        Self::resolve_package(&property.name)
            .unwrap_or("unknown")
            .to_string()
    }

    fn resolve<'a>(
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
    use std::path::PathBuf;

    #[test]
    fn patterns_match_pm_tools() {
        let resolver = PmVersionsResolver::new();
        let patterns = resolver.patterns();
        assert!(patterns.contains(&"version.npm"));
        assert!(patterns.contains(&"npm.version"));
        assert!(patterns.contains(&"version.pnpm"));
        assert!(patterns.contains(&"pnpm.version"));
        assert!(patterns.contains(&"version.yarn"));
        assert!(patterns.contains(&"yarn.version"));
    }

    #[test]
    fn resolve_package_names() {
        assert_eq!(
            PmVersionsResolver::resolve_package("version.npm"),
            Some("npm")
        );
        assert_eq!(
            PmVersionsResolver::resolve_package("npm.version"),
            Some("npm")
        );
        assert_eq!(
            PmVersionsResolver::resolve_package("version.pnpm"),
            Some("pnpm")
        );
        assert_eq!(
            PmVersionsResolver::resolve_package("pnpm.version"),
            Some("pnpm")
        );
        assert_eq!(
            PmVersionsResolver::resolve_package("version.yarn"),
            Some("yarn")
        );
        assert_eq!(
            PmVersionsResolver::resolve_package("yarn.version"),
            Some("yarn")
        );
    }

    #[test]
    fn does_not_match_unrelated() {
        assert_eq!(PmVersionsResolver::resolve_package("version.junit"), None);
        assert_eq!(PmVersionsResolver::resolve_package("npm"), None);
        assert_eq!(PmVersionsResolver::resolve_package("version.node"), None);
    }

    #[test]
    fn label_resolves_from_property() {
        let resolver = PmVersionsResolver::new();
        let prop = VersionProperty {
            name: "version.pnpm".to_string(),
            current_value: "9.0.0".to_string(),
            source: PathBuf::from("pom.xml"),
        };
        assert_eq!(resolver.label(&prop), "pnpm");
    }

    #[test]
    fn label_returns_unknown_for_unrecognized_property() {
        let resolver = PmVersionsResolver::new();
        let prop = VersionProperty {
            name: "version.unknown-tool".to_string(),
            current_value: "1.0.0".to_string(),
            source: PathBuf::from("pom.xml"),
        };
        assert_eq!(resolver.label(&prop), "unknown");
    }

    #[test]
    fn resolve_package_returns_none_for_node() {
        // version.node is handled by NodeResolver, not PmVersionsResolver
        assert_eq!(PmVersionsResolver::resolve_package("version.node"), None);
    }
}
