use std::fs;
use std::path::{Path, PathBuf};

use super::PackageManager;

#[derive(Debug, Clone)]
pub struct NpmProject {
    pub path: PathBuf,
    pub name: String,
    pub package_manager: PackageManager,
}

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
            projects.push(NpmProject {
                path: dir.clone(),
                name,
                package_manager: pm,
            });
        }
    }
    projects
}

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

fn detect_from_package_manager_field(dir: &Path) -> Option<PackageManager> {
    let content = fs::read_to_string(dir.join("package.json")).ok()?;
    let pkg: serde_json::Value = serde_json::from_str(&content).ok()?;
    let pm_field = pkg.get("packageManager")?.as_str()?;

    if pm_field.starts_with("pnpm@") {
        Some(PackageManager::Pnpm)
    } else if pm_field.starts_with("npm@") {
        Some(PackageManager::Npm)
    } else if pm_field.starts_with("yarn@") {
        Some(PackageManager::Yarn)
    } else if pm_field.starts_with("bun@") {
        Some(PackageManager::Bun)
    } else {
        None
    }
}

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
        if name == "node_modules" || name == ".git" || name == "target" {
            continue;
        }
        collect_package_dirs(&path, result);
    }
}

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
        fs::write(
            dir.join("package.json"),
            format!(r#"{{"name": "{name}"}}"#),
        )
        .unwrap();
        fs::write(dir.join(lock_file), "").unwrap();
    }

    fn setup_bare_project(dir: &Path, name: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(
            dir.join("package.json"),
            format!(r#"{{"name": "{name}"}}"#),
        )
        .unwrap();
    }

    // -- lock file detection --

    #[test]
    fn detects_pnpm_by_lockfile() {
        let tmp = tempfile::tempdir().unwrap();
        setup_project(tmp.path(), "my-app", "pnpm-lock.yaml");

        let projects = discover(tmp.path());
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].package_manager, PackageManager::Pnpm);
    }

    #[test]
    fn detects_npm_by_lockfile() {
        let tmp = tempfile::tempdir().unwrap();
        setup_project(tmp.path(), "my-app", "package-lock.json");

        let projects = discover(tmp.path());
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].package_manager, PackageManager::Npm);
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
        fs::write(
            member.join("package.json"),
            r#"{"name": "child"}"#,
        )
        .unwrap();
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
