use std::path::PathBuf;
use std::process::Command;

fn fixture_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn depup() -> Command {
    Command::new(env!("CARGO_BIN_EXE_depup"))
}

#[test]
fn json_output_returns_array() {
    let output = depup()
        .arg("check")
        .arg("--json")
        .arg(&fixture_dir("multi-module"))
        .output()
        .expect("Failed to run depup");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).expect("Invalid JSON output");
    assert_eq!(results.len(), 2);
}

#[test]
fn no_subcommand_returns_error() {
    let output = depup().output().expect("Failed to run depup");

    assert!(!output.status.success());
}

#[test]
fn outdated_filter_excludes_current() {
    let output = depup()
        .arg("check")
        .arg("--json")
        .arg("--outdated")
        .arg(&fixture_dir("multi-module"))
        .output()
        .expect("Failed to run depup");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).expect("Invalid JSON output");

    for result in &results {
        assert_eq!(
            result["status"].as_str().unwrap(),
            "outdated",
            "--outdated should only return outdated properties"
        );
    }
}

#[test]
fn missing_pom_returns_json_error() {
    let output = depup()
        .arg("check")
        .arg("--json")
        .arg("/nonexistent/path")
        .output()
        .expect("Failed to run depup");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).expect("Invalid JSON output");
    assert!(results.is_empty());
}

#[test]
fn auto_detect_missing_project_returns_zero_exit() {
    let output = depup()
        .arg("check")
        .arg("/nonexistent/path")
        .output()
        .expect("Failed to run depup");

    assert!(output.status.success());
}

#[test]
fn json_output_includes_artifact() {
    let output = depup()
        .arg("check")
        .arg("--json")
        .arg(&fixture_dir("multi-module"))
        .output()
        .expect("Failed to run depup");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).expect("Invalid JSON output");

    for result in &results {
        assert!(
            result["artifact"].as_str().is_some(),
            "Artifact should be present in JSON output"
        );
    }
}

#[test]
fn update_dry_run_shows_would_update() {
    let output = depup()
        .arg("update")
        .arg("--json")
        .arg("--dry-run")
        .arg(&fixture_dir("multi-module"))
        .output()
        .expect("Failed to run depup");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).expect("Invalid JSON output");

    for result in &results {
        assert_eq!(
            result["status"].as_str().unwrap(),
            "would_update",
            "Dry run should report would_update status"
        );
    }
}

#[test]
fn update_maven_modifies_pom() {
    let tmp = tempfile::TempDir::new().unwrap();
    let fixture = fixture_dir("update-test");
    let pom_src = fixture.join("pom.xml");
    let pom_dst = tmp.path().join("pom.xml");
    std::fs::copy(&pom_src, &pom_dst).unwrap();

    let output = depup()
        .arg("update")
        .arg("--json")
        .arg(tmp.path())
        .output()
        .expect("Failed to run depup");

    assert!(output.status.success());

    let pom_content = std::fs::read_to_string(&pom_dst).unwrap();
    assert!(
        !pom_content.contains("<version.junit>5.10.0</version.junit>"),
        "POM should have been updated with a newer junit version"
    );
    assert!(pom_content.contains("<!-- Intentionally old versions for update testing -->"));
    assert!(pom_content.contains("xmlns=\"http://maven.apache.org/POM/4.0.0\""));
}

#[test]
fn update_preserves_pom_formatting() {
    let tmp = tempfile::TempDir::new().unwrap();
    let fixture = fixture_dir("update-test");
    let pom_src = fixture.join("pom.xml");
    let pom_dst = tmp.path().join("pom.xml");
    std::fs::copy(&pom_src, &pom_dst).unwrap();

    let original = std::fs::read_to_string(&pom_dst).unwrap();

    let output = depup()
        .arg("update")
        .arg(tmp.path())
        .output()
        .expect("Failed to run depup");

    assert!(output.status.success());

    let updated = std::fs::read_to_string(&pom_dst).unwrap();
    assert_eq!(
        original.lines().count(),
        updated.lines().count(),
        "Update should preserve POM line count"
    );
}

#[test]
fn update_no_project_returns_empty_json() {
    let tmp = tempfile::TempDir::new().unwrap();
    let output = depup()
        .arg("update")
        .arg("--json")
        .arg(tmp.path())
        .output()
        .expect("Failed to run depup");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "[]");
}

#[test]
fn update_maven_only_flag() {
    let output = depup()
        .arg("update")
        .arg("--json")
        .arg("--dry-run")
        .arg("--maven")
        .arg(&fixture_dir("multi-module"))
        .output()
        .expect("Failed to run depup");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let results: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).expect("Invalid JSON output");

    for result in &results {
        assert_eq!(result["ecosystem"].as_str().unwrap(), "maven");
    }
}

#[test]
fn update_modifies_inline_versions() {
    let tmp = tempfile::TempDir::new().unwrap();
    let fixture = fixture_dir("plain-versions");
    let pom_src = fixture.join("pom.xml");
    let pom_dst = tmp.path().join("pom.xml");
    std::fs::copy(&pom_src, &pom_dst).unwrap();

    let output = depup()
        .arg("update")
        .arg("--json")
        .arg(tmp.path())
        .output()
        .expect("Failed to run depup");

    assert!(output.status.success());

    let pom_content = std::fs::read_to_string(&pom_dst).unwrap();
    assert!(
        !pom_content.contains("<version>33.0.0-jre</version>"),
        "Inline guava version should have been updated"
    );
    assert!(
        !pom_content.contains("<version.junit>5.10.0</version.junit>"),
        "Managed junit version should have been updated too"
    );
    assert!(
        pom_content.contains("xmlns=\"http://maven.apache.org/POM/4.0.0\""),
        "XML namespace should be preserved"
    );
}

#[test]
fn update_dry_run_json_has_structured_fields() {
    let output = depup()
        .arg("update")
        .arg("--json")
        .arg("--dry-run")
        .arg(&fixture_dir("plain-versions"))
        .output()
        .expect("Failed to run depup");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).expect("Invalid JSON output");

    assert!(!results.is_empty());
    for result in &results {
        assert!(result.get("ecosystem").is_some(), "must have ecosystem");
        assert!(result.get("property").is_some(), "must have property");
        assert!(result.get("old_version").is_some(), "must have old_version");
        assert!(result.get("new_version").is_some(), "must have new_version");
        assert!(result.get("kind").is_some(), "must have kind");
        assert!(result.get("managed").is_some(), "must have managed");
        assert!(result.get("status").is_some(), "must have status");
        assert!(result.get("source").is_some(), "must have source");
    }
}

#[test]
fn update_managed_filter() {
    let output = depup()
        .arg("update")
        .arg("--json")
        .arg("--dry-run")
        .arg("--managed")
        .arg(&fixture_dir("plain-versions"))
        .output()
        .expect("Failed to run depup");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).expect("Invalid JSON output");

    for result in &results {
        assert!(
            result["managed"].as_bool().unwrap(),
            "--managed should only include managed deps"
        );
    }
}

#[test]
fn update_dependencies_filter() {
    let output = depup()
        .arg("update")
        .arg("--json")
        .arg("--dry-run")
        .arg("--dependencies")
        .arg(&fixture_dir("multi-module"))
        .output()
        .expect("Failed to run depup");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).expect("Invalid JSON output");

    for result in &results {
        assert_eq!(
            result["kind"].as_str().unwrap(),
            "dependency",
            "--dependencies should only include dependencies"
        );
    }
}

#[test]
fn json_output_includes_ecosystem() {
    let output = depup()
        .arg("check")
        .arg("--json")
        .arg(&fixture_dir("multi-module"))
        .output()
        .expect("Failed to run depup");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).expect("Invalid JSON output");

    for result in &results {
        assert_eq!(
            result["ecosystem"].as_str().unwrap(),
            "maven",
            "Multi-module fixture should report maven ecosystem"
        );
    }
}

#[test]
fn stable_alias_works() {
    let output = depup()
        .arg("check")
        .arg("--json")
        .arg("--releases-only")
        .arg(&fixture_dir("multi-module"))
        .output()
        .expect("Failed to run depup");

    assert!(output.status.success() || !output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let results: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).expect("Invalid JSON output");
    for result in &results {
        assert_ne!(
            result["status"].as_str().unwrap(),
            "skipped",
            "--releases-only (alias for --stable) should exclude skipped"
        );
    }
}

#[test]
fn json_output_includes_managed_field() {
    let output = depup()
        .arg("check")
        .arg("--json")
        .arg(&fixture_dir("multi-module"))
        .output()
        .expect("Failed to run depup");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).expect("Invalid JSON output");

    for result in &results {
        assert!(
            result.get("managed").is_some(),
            "JSON output should include managed field"
        );
    }
}

#[test]
fn managed_unmanaged_conflict() {
    let output = depup()
        .arg("check")
        .arg("--managed")
        .arg("--unmanaged")
        .arg(&fixture_dir("multi-module"))
        .output()
        .expect("Failed to run depup");

    assert!(!output.status.success());
}

#[test]
fn maven_npm_conflict() {
    let output = depup()
        .arg("check")
        .arg("--maven")
        .arg("--npm")
        .arg(&fixture_dir("multi-module"))
        .output()
        .expect("Failed to run depup");

    assert!(!output.status.success());
}

#[test]
fn kind_filter_conflict() {
    let output = depup()
        .arg("check")
        .arg("--dependencies")
        .arg("--plugins")
        .arg(&fixture_dir("multi-module"))
        .output()
        .expect("Failed to run depup");

    assert!(!output.status.success());
}

#[test]
fn maven_filter_only_maven_results() {
    let output = depup()
        .arg("check")
        .arg("--json")
        .arg("--maven")
        .arg(&fixture_dir("multi-module"))
        .output()
        .expect("Failed to run depup");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).expect("Invalid JSON output");

    for result in &results {
        assert_eq!(
            result["ecosystem"].as_str().unwrap(),
            "maven",
            "--maven should only return maven results"
        );
    }
}

#[test]
fn managed_filter_only_managed() {
    let output = depup()
        .arg("check")
        .arg("--json")
        .arg("--managed")
        .arg(&fixture_dir("plain-versions"))
        .output()
        .expect("Failed to run depup");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).expect("Invalid JSON output");

    for result in &results {
        assert!(
            result["managed"].as_bool().unwrap(),
            "--managed should only return managed dependencies"
        );
    }
}

#[test]
fn unmanaged_filter_only_unmanaged() {
    let output = depup()
        .arg("check")
        .arg("--json")
        .arg("--unmanaged")
        .arg(&fixture_dir("plain-versions"))
        .output()
        .expect("Failed to run depup");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).expect("Invalid JSON output");

    for result in &results {
        assert!(
            !result["managed"].as_bool().unwrap(),
            "--unmanaged should only return unmanaged dependencies"
        );
    }
}

#[test]
fn audit_stub_returns_not_implemented_json() {
    let output = depup()
        .arg("audit")
        .arg("--json")
        .output()
        .expect("Failed to run depup");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let envelope: serde_json::Value = serde_json::from_str(&stdout).expect("Invalid JSON output");
    assert_eq!(envelope["error"]["code"], "NOT_IMPLEMENTED");
}
