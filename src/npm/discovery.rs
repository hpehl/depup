//! npm ecosystem project discovery.
//!
//! Walks a directory tree finding npm ecosystem projects. Detects the package
//! manager by lock file (`pnpm-lock.yaml`, `package-lock.json`, `yarn.lock`,
//! `bun.lock`/`bun.lockb`) or `packageManager` field in `package.json`.
//! Skips directories listed in [`SKIP_DIRS`] and workspace members.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

/// Directories to skip during project discovery. These are package manager
/// internals, caches, build outputs, and VCS metadata — never real projects.
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    ".pnpm-store",
    ".yarn",
    ".bun",
    ".git",
    "target",
    "dist",
    "build",
];

use super::PackageManager;

/// A discovered npm ecosystem project with its path, name, and detected package manager.
#[derive(Debug, Clone)]
pub struct NpmProject {
    pub path: PathBuf,
    pub name: String,
    pub package_manager: PackageManager,
    /// Version from the `packageManager` field in `package.json` (e.g., `"9.15.0"` from `"pnpm@9.15.0"`).
    pub pm_version: Option<String>,
}

/// Discovers all npm ecosystem projects under the given root directory.
/// Workspace members are excluded — only workspace roots are returned.
pub fn discover(root: &Path) -> Vec<NpmProject> {
    let mut all_dirs = Vec::new();
    collect_package_dirs(root, &mut all_dirs);
    all_dirs.sort();

    let workspace_roots: Vec<(&PathBuf, PackageManager)> = all_dirs
        .iter()
        .filter_map(|dir| {
            let pm = detect_package_manager(dir)?;
            if is_workspace_root(dir, pm) {
                Some((dir, pm))
            } else {
                None
            }
        })
        .collect();

    let mut projects = Vec::new();
    for dir in &all_dirs {
        if is_workspace_member(dir, &workspace_roots) {
            continue;
        }
        if let Some(pm) = detect_package_manager(dir) {
            let name = read_package_name(dir).unwrap_or_else(|| dir.display().to_string());
            let pm_version = read_pm_version(dir);
            projects.push(NpmProject {
                path: dir.clone(),
                name,
                package_manager: pm,
                pm_version,
            });
        }
    }
    projects
}

/// Detects the package manager by lock file presence, falling back to the
/// `packageManager` field in `package.json`.
fn detect_package_manager(dir: &Path) -> Option<PackageManager> {
    if dir.join("pnpm-lock.yaml").exists() {
        return Some(PackageManager::Pnpm);
    }
    if dir.join("package-lock.json").exists() {
        return Some(PackageManager::Npm);
    }
    if dir.join("yarn.lock").exists() {
        return Some(PackageManager::Yarn);
    }
    if dir.join("bun.lock").exists() || dir.join("bun.lockb").exists() {
        return Some(PackageManager::Bun);
    }
    detect_from_package_manager_field(dir)
}

/// Reads the `packageManager` field from `package.json` (e.g., `"pnpm@9.15.0"`)
/// and returns the detected package manager.
fn detect_from_package_manager_field(dir: &Path) -> Option<PackageManager> {
    parse_package_manager_field(dir).map(|(pm, _)| pm)
}

/// Reads the version from the `packageManager` field in `package.json`.
/// Strips any `+hash` suffix (Corepack format: `pnpm@9.15.0+sha512.abc...`).
fn read_pm_version(dir: &Path) -> Option<String> {
    parse_package_manager_field(dir).map(|(_, version)| version)
}

/// Parses the `packageManager` field from `package.json`, returning the
/// detected package manager and version. Strips `+hash` suffixes.
fn parse_package_manager_field(dir: &Path) -> Option<(PackageManager, String)> {
    let content = fs::read_to_string(dir.join("package.json")).ok()?;
    let pkg: serde_json::Value = serde_json::from_str(&content).ok()?;
    let pm_field = pkg.get("packageManager")?.as_str()?;

    let (name, version) = pm_field.split_once('@')?;
    let version = version.split_once('+').map_or(version, |(v, _)| v);

    let pm = match name {
        "pnpm" => PackageManager::Pnpm,
        "npm" => PackageManager::Npm,
        "yarn" => PackageManager::Yarn,
        "bun" => PackageManager::Bun,
        _ => return None,
    };
    Some((pm, version.to_string()))
}

/// Checks if a directory is a workspace root (pnpm: `pnpm-workspace.yaml`,
/// npm/yarn/bun: `workspaces` field in `package.json`).
fn is_workspace_root(dir: &Path, pm: PackageManager) -> bool {
    match pm {
        PackageManager::Pnpm => dir.join("pnpm-workspace.yaml").exists(),
        PackageManager::Npm | PackageManager::Yarn | PackageManager::Bun => {
            if let Ok(content) = fs::read_to_string(dir.join("package.json"))
                && let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content)
            {
                pkg.get("workspaces").is_some()
            } else {
                false
            }
        }
    }
}

/// Recursively collects directories containing `package.json`.
/// Skips directories listed in [`SKIP_DIRS`].
fn collect_package_dirs(dir: &Path, result: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            if path.file_name().is_some_and(|n| n == "package.json") {
                result.push(dir.to_path_buf());
            }
            continue;
        }
        let name = entry.file_name();
        if SKIP_DIRS.iter().any(|d| name == OsStr::new(d)) {
            continue;
        }
        collect_package_dirs(&path, result);
    }
}

/// Returns true if `dir` is a subdirectory of any workspace root (but not the root itself).
fn is_workspace_member(dir: &Path, workspace_roots: &[(&PathBuf, PackageManager)]) -> bool {
    workspace_roots
        .iter()
        .any(|(root, _)| dir != root.as_path() && dir.starts_with(root))
}

fn read_package_name(dir: &Path) -> Option<String> {
    let content = fs::read_to_string(dir.join("package.json")).ok()?;
    let pkg: serde_json::Value = serde_json::from_str(&content).ok()?;
    pkg.get("name").and_then(|v| v.as_str()).map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_project(dir: &Path, name: &str, lock_file: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("package.json"), format!(r#"{{"name": "{name}"}}"#)).unwrap();
        fs::write(dir.join(lock_file), "").unwrap();
    }

    fn setup_bare_project(dir: &Path, name: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("package.json"), format!(r#"{{"name": "{name}"}}"#)).unwrap();
    }

    // -- lock file detection --

    #[test]
    fn detects_pnpm_by_lockfile() {
        let tmp = tempfile::tempdir().unwrap();
        setup_project(tmp.path(), "my-app", "pnpm-lock.yaml");

        let projects = discover(tmp.path());
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].package_manager, PackageManager::Pnpm);
        assert_eq!(projects[0].pm_version, None);
    }

    #[test]
    fn detects_npm_by_lockfile() {
        let tmp = tempfile::tempdir().unwrap();
        setup_project(tmp.path(), "my-app", "package-lock.json");

        let projects = discover(tmp.path());
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].package_manager, PackageManager::Npm);
        assert_eq!(projects[0].pm_version, None);
    }

    #[test]
    fn detects_yarn_by_lockfile() {
        let tmp = tempfile::tempdir().unwrap();
        setup_project(tmp.path(), "my-app", "yarn.lock");

        let projects = discover(tmp.path());
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].package_manager, PackageManager::Yarn);
    }

    #[test]
    fn detects_bun_by_lockfile() {
        let tmp = tempfile::tempdir().unwrap();
        setup_project(tmp.path(), "my-app", "bun.lock");

        let projects = discover(tmp.path());
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].package_manager, PackageManager::Bun);
    }

    #[test]
    fn detects_bun_by_binary_lockfile() {
        let tmp = tempfile::tempdir().unwrap();
        setup_project(tmp.path(), "my-app", "bun.lockb");

        let projects = discover(tmp.path());
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].package_manager, PackageManager::Bun);
    }

    #[test]
    fn lockfile_with_package_manager_field_captures_version() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("package.json"),
            r#"{"name": "my-app", "packageManager": "pnpm@9.15.0"}"#,
        )
        .unwrap();
        fs::write(tmp.path().join("pnpm-lock.yaml"), "").unwrap();

        let projects = discover(tmp.path());
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].package_manager, PackageManager::Pnpm);
        assert_eq!(projects[0].pm_version, Some("9.15.0".to_string()));
    }

    // -- packageManager field fallback --

    #[test]
    fn detects_pnpm_by_package_manager_field() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("package.json"),
            r#"{"name": "pm-app", "packageManager": "pnpm@9.15.0"}"#,
        )
        .unwrap();

        let projects = discover(tmp.path());
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].package_manager, PackageManager::Pnpm);
        assert_eq!(projects[0].pm_version, Some("9.15.0".to_string()));
    }

    #[test]
    fn detects_npm_by_package_manager_field() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("package.json"),
            r#"{"name": "npm-app", "packageManager": "npm@10.0.0"}"#,
        )
        .unwrap();

        let projects = discover(tmp.path());
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].package_manager, PackageManager::Npm);
        assert_eq!(projects[0].pm_version, Some("10.0.0".to_string()));
    }

    #[test]
    fn detects_yarn_by_package_manager_field() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("package.json"),
            r#"{"name": "yarn-app", "packageManager": "yarn@4.0.0"}"#,
        )
        .unwrap();

        let projects = discover(tmp.path());
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].package_manager, PackageManager::Yarn);
        assert_eq!(projects[0].pm_version, Some("4.0.0".to_string()));
    }

    #[test]
    fn detects_bun_by_package_manager_field() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("package.json"),
            r#"{"name": "bun-app", "packageManager": "bun@1.1.0"}"#,
        )
        .unwrap();

        let projects = discover(tmp.path());
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].package_manager, PackageManager::Bun);
        assert_eq!(projects[0].pm_version, Some("1.1.0".to_string()));
    }

    #[test]
    fn strips_hash_suffix_from_pm_version() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("package.json"),
            r#"{"name": "hash-app", "packageManager": "pnpm@9.15.0+sha512.abc123"}"#,
        )
        .unwrap();

        let projects = discover(tmp.path());
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].package_manager, PackageManager::Pnpm);
        assert_eq!(projects[0].pm_version, Some("9.15.0".to_string()));
    }

    // -- parse_package_manager_field unit tests --

    #[test]
    fn parse_pm_field_returns_none_for_unknown_pm() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("package.json"),
            r#"{"name": "app", "packageManager": "deno@1.0.0"}"#,
        )
        .unwrap();
        assert!(parse_package_manager_field(tmp.path()).is_none());
    }

    #[test]
    fn parse_pm_field_returns_none_without_at() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("package.json"),
            r#"{"name": "app", "packageManager": "pnpm"}"#,
        )
        .unwrap();
        assert!(parse_package_manager_field(tmp.path()).is_none());
    }

    // -- skips and workspace --

    #[test]
    fn skips_bare_project_without_lock_or_pm_field() {
        let tmp = tempfile::tempdir().unwrap();
        setup_bare_project(tmp.path(), "bare-app");

        let projects = discover(tmp.path());
        assert!(projects.is_empty());
    }

    #[test]
    fn skips_pnpm_workspace_members() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        setup_project(root, "workspace-root", "pnpm-lock.yaml");
        fs::write(
            root.join("pnpm-workspace.yaml"),
            "packages:\n  - 'packages/*'\n",
        )
        .unwrap();

        let member = root.join("packages").join("member-a");
        setup_project(&member, "member-a", "pnpm-lock.yaml");

        let projects = discover(root);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "workspace-root");
    }

    #[test]
    fn skips_npm_workspace_members() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        fs::create_dir_all(root).unwrap();
        fs::write(
            root.join("package.json"),
            r#"{"name": "mono", "workspaces": ["packages/*"]}"#,
        )
        .unwrap();
        fs::write(root.join("package-lock.json"), "{}").unwrap();

        let member = root.join("packages").join("child");
        fs::create_dir_all(&member).unwrap();
        fs::write(member.join("package.json"), r#"{"name": "child"}"#).unwrap();
        fs::write(member.join("package-lock.json"), "{}").unwrap();

        let projects = discover(root);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "mono");
    }

    #[test]
    fn discovers_multiple_projects() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        setup_project(&root.join("app1"), "app1", "pnpm-lock.yaml");
        setup_project(&root.join("app2"), "app2", "package-lock.json");

        let projects = discover(root);
        assert_eq!(projects.len(), 2);
    }

    #[test]
    fn skips_pnpm_store() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        setup_project(root, "my-app", "pnpm-lock.yaml");
        setup_project(
            &root.join(".pnpm-store").join("v3").join("some-pkg"),
            "some-pkg",
            "pnpm-lock.yaml",
        );

        let projects = discover(root);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "my-app");
    }

    #[test]
    fn skips_node_modules() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        setup_project(root, "my-app", "pnpm-lock.yaml");
        setup_project(
            &root.join("node_modules").join("some-dep"),
            "some-dep",
            "package-lock.json",
        );

        let projects = discover(root);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "my-app");
    }
}
