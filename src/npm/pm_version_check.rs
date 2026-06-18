//! Package manager version checking and updating for the `packageManager` field
//! in `package.json`.
//!
//! Queries the npm registry for the latest version of the detected package manager
//! and can rewrite the `packageManager` field in `package.json` when updating.

use std::path::Path;

use anyhow::Result;

use super::discovery::NpmProject;
use crate::constants::{self, NPM_REGISTRY_URL};
use crate::model::{CheckResult, Dependency, DependencyKind, Ecosystem};
use crate::version;

/// Checks the project's `packageManager` version against the npm registry.
/// Returns `None` if no `pm_version` is set on the project.
pub async fn check_pm_version(project: &NpmProject, source: &str) -> Option<CheckResult> {
    let current = project.pm_version.as_ref()?;
    let pm_name = project.package_manager.command();
    Some(fetch_and_check(pm_name, current, source).await)
}

async fn fetch_and_check(pm_name: &str, current: &str, source: &str) -> CheckResult {
    let id = Dependency::new(
        Ecosystem::Npm,
        DependencyKind::Tool,
        pm_name.to_string(),
        None,
        source.to_string(),
    );

    let url = format!("{NPM_REGISTRY_URL}/{pm_name}");
    let body: serde_json::Value = match constants::fetch_json(&url).await {
        Ok(v) => v,
        Err(e) => {
            return CheckResult::error(id, current.to_string(), e.to_string());
        }
    };

    match body["dist-tags"]["latest"].as_str() {
        Some(latest) => {
            let is_outdated = version::is_newer(current, latest);
            CheckResult::checked(id, current.to_string(), latest.to_string(), is_outdated)
        }
        None => CheckResult::error(
            id,
            current.to_string(),
            format!("No latest version found for {pm_name}"),
        ),
    }
}

/// Rewrites the `packageManager` field in `package.json` to use the new version.
/// Preserves the `name@` prefix and any existing formatting.
pub fn update_pm_version(project_path: &Path, pm_name: &str, new_version: &str) -> Result<()> {
    let pkg_path = project_path.join("package.json");
    let content = std::fs::read_to_string(&pkg_path)?;
    let mut pkg: serde_json::Value = serde_json::from_str(&content)?;

    if let Some(field) = pkg.get_mut("packageManager") {
        *field = serde_json::Value::String(format!("{pm_name}@{new_version}"));
    }

    let output = serde_json::to_string_pretty(&pkg)? + "\n";
    std::fs::write(&pkg_path, output)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn update_pm_version_rewrites_field() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("package.json"),
            r#"{"name": "app", "packageManager": "pnpm@9.15.0"}"#,
        )
        .unwrap();

        update_pm_version(tmp.path(), "pnpm", "10.0.0").unwrap();

        let content = fs::read_to_string(tmp.path().join("package.json")).unwrap();
        let pkg: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(pkg["packageManager"], "pnpm@10.0.0");
    }

    #[test]
    fn update_pm_version_strips_hash_suffix() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("package.json"),
            r#"{"name": "app", "packageManager": "pnpm@9.15.0+sha512.abc123"}"#,
        )
        .unwrap();

        update_pm_version(tmp.path(), "pnpm", "10.0.0").unwrap();

        let content = fs::read_to_string(tmp.path().join("package.json")).unwrap();
        let pkg: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(pkg["packageManager"], "pnpm@10.0.0");
    }

    #[test]
    fn update_pm_version_preserves_other_fields() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("package.json"),
            r#"{"name": "app", "version": "1.0.0", "packageManager": "npm@10.0.0"}"#,
        )
        .unwrap();

        update_pm_version(tmp.path(), "npm", "11.0.0").unwrap();

        let content = fs::read_to_string(tmp.path().join("package.json")).unwrap();
        let pkg: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(pkg["name"], "app");
        assert_eq!(pkg["version"], "1.0.0");
        assert_eq!(pkg["packageManager"], "npm@11.0.0");
    }

    #[test]
    fn update_pm_version_missing_file_errors() {
        let tmp = TempDir::new().unwrap();
        let result = update_pm_version(tmp.path(), "pnpm", "10.0.0");
        assert!(result.is_err());
    }
}
