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
/// Uses string-level replacement to preserve key order and formatting.
pub fn update_pm_version(project_path: &Path, pm_name: &str, new_version: &str) -> Result<()> {
    let pkg_path = project_path.join("package.json");
    let content = std::fs::read_to_string(&pkg_path)?;

    let new_value = format!("{pm_name}@{new_version}");
    let updated = replace_pm_value(&content, &new_value).ok_or_else(|| {
        anyhow::anyhow!("packageManager field not found in {}", pkg_path.display())
    })?;

    std::fs::write(&pkg_path, updated)?;
    Ok(())
}

/// Replaces the value of the `"packageManager"` field in raw JSON content,
/// preserving all other formatting, key order, and whitespace.
fn replace_pm_value(content: &str, new_value: &str) -> Option<String> {
    let key = "\"packageManager\"";
    let key_pos = content.find(key)?;
    let after_key = key_pos + key.len();

    let colon_offset = content[after_key..].find(':')?;
    let after_colon = after_key + colon_offset + 1;

    let quote_start_offset = content[after_colon..].find('"')?;
    let value_start = after_colon + quote_start_offset + 1;

    let quote_end_offset = content[value_start..].find('"')?;
    let value_end = value_start + quote_end_offset;

    let mut result = String::with_capacity(content.len());
    result.push_str(&content[..value_start]);
    result.push_str(new_value);
    result.push_str(&content[value_end..]);
    Some(result)
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
    fn update_pm_version_preserves_key_order() {
        let tmp = TempDir::new().unwrap();
        let original = r#"{
  "name": "app",
  "version": "1.0.0",
  "scripts": {
    "build": "tsc"
  },
  "packageManager": "pnpm@9.15.0",
  "dependencies": {
    "react": "^18.0.0"
  }
}
"#;
        fs::write(tmp.path().join("package.json"), original).unwrap();

        update_pm_version(tmp.path(), "pnpm", "10.0.0").unwrap();

        let content = fs::read_to_string(tmp.path().join("package.json")).unwrap();
        let expected = original.replace("pnpm@9.15.0", "pnpm@10.0.0");
        assert_eq!(content, expected);
    }

    #[test]
    fn update_pm_version_missing_file_errors() {
        let tmp = TempDir::new().unwrap();
        let result = update_pm_version(tmp.path(), "pnpm", "10.0.0");
        assert!(result.is_err());
    }

    #[test]
    fn replace_pm_value_basic() {
        let input = r#"{"packageManager": "pnpm@9.0.0"}"#;
        let result = replace_pm_value(input, "pnpm@10.0.0").unwrap();
        assert_eq!(result, r#"{"packageManager": "pnpm@10.0.0"}"#);
    }

    #[test]
    fn replace_pm_value_with_hash_suffix() {
        let input = r#"{"packageManager": "pnpm@9.0.0+sha512.abc"}"#;
        let result = replace_pm_value(input, "pnpm@10.0.0").unwrap();
        assert_eq!(result, r#"{"packageManager": "pnpm@10.0.0"}"#);
    }

    #[test]
    fn replace_pm_value_preserves_surrounding_content() {
        let input = r#"{
  "name": "test",
  "packageManager": "npm@9.0.0",
  "version": "1.0.0"
}"#;
        let expected = r#"{
  "name": "test",
  "packageManager": "npm@10.0.0",
  "version": "1.0.0"
}"#;
        let result = replace_pm_value(input, "npm@10.0.0").unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn replace_pm_value_not_found() {
        let input = r#"{"name": "test"}"#;
        assert!(replace_pm_value(input, "npm@10.0.0").is_none());
    }
}
