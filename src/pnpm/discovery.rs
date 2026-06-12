use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct PnpmProject {
    pub path: PathBuf,
    pub name: String,
}

pub fn discover(root: &Path) -> Vec<PnpmProject> {
    let mut all_dirs = Vec::new();
    collect_package_dirs(root, &mut all_dirs);
    all_dirs.sort();

    let workspace_roots: Vec<&PathBuf> = all_dirs
        .iter()
        .filter(|dir| dir.join("pnpm-workspace.yaml").exists() && is_pnpm_project(dir))
        .collect();

    let mut projects = Vec::new();
    for dir in &all_dirs {
        if is_workspace_member(dir, &workspace_roots) {
            continue;
        }
        if is_pnpm_project(dir) {
            let name = read_package_name(dir).unwrap_or_else(|| dir.display().to_string());
            projects.push(PnpmProject {
                path: dir.clone(),
                name,
            });
        }
    }
    projects
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

pub fn is_pnpm_project(dir: &Path) -> bool {
    if dir.join("pnpm-lock.yaml").exists() {
        return true;
    }
    if let Ok(content) = fs::read_to_string(dir.join("package.json"))
        && let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content)
        && let Some(pm) = pkg.get("packageManager").and_then(|v| v.as_str())
    {
        return pm.starts_with("pnpm@");
    }
    false
}

fn is_workspace_member(dir: &Path, workspace_roots: &[&PathBuf]) -> bool {
    for root in workspace_roots {
        if dir != root.as_path() && dir.starts_with(root) {
            return true;
        }
    }
    false
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

    fn setup_pnpm_project(dir: &Path, name: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(
            dir.join("package.json"),
            format!(r#"{{"name": "{name}"}}"#),
        )
        .unwrap();
        fs::write(dir.join("pnpm-lock.yaml"), "lockfileVersion: '9.0'\n").unwrap();
    }

    fn setup_non_pnpm_project(dir: &Path, name: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(
            dir.join("package.json"),
            format!(r#"{{"name": "{name}"}}"#),
        )
        .unwrap();
    }

    #[test]
    fn discovers_pnpm_project_by_lockfile() {
        let tmp = tempfile::tempdir().unwrap();
        setup_pnpm_project(tmp.path(), "my-app");

        let projects = discover(tmp.path());
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "my-app");
    }

    #[test]
    fn discovers_pnpm_project_by_package_manager_field() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::write(
            dir.join("package.json"),
            r#"{"name": "pm-app", "packageManager": "pnpm@9.15.0"}"#,
        )
        .unwrap();

        let projects = discover(dir);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "pm-app");
    }

    #[test]
    fn skips_non_pnpm_project() {
        let tmp = tempfile::tempdir().unwrap();
        setup_non_pnpm_project(tmp.path(), "npm-app");

        let projects = discover(tmp.path());
        assert!(projects.is_empty());
    }

    #[test]
    fn skips_workspace_members() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        setup_pnpm_project(root, "workspace-root");
        fs::write(root.join("pnpm-workspace.yaml"), "packages:\n  - 'packages/*'\n").unwrap();

        let member = root.join("packages").join("member-a");
        setup_pnpm_project(&member, "member-a");

        let projects = discover(root);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "workspace-root");
    }

    #[test]
    fn discovers_multiple_projects() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        let app1 = root.join("app1");
        setup_pnpm_project(&app1, "app1");

        let app2 = root.join("app2");
        setup_pnpm_project(&app2, "app2");

        let projects = discover(root);
        assert_eq!(projects.len(), 2);
    }

    #[test]
    fn skips_node_modules() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        setup_pnpm_project(root, "my-app");

        let nested = root.join("node_modules").join("some-dep");
        setup_pnpm_project(&nested, "some-dep");

        let projects = discover(root);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "my-app");
    }
}
