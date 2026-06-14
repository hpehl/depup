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
        .arg(&fixture_dir("multi-module"))
        .arg("--json")
        .output()
        .expect("Failed to run depup");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).expect("Invalid JSON output");
    assert_eq!(results.len(), 2);
}

#[test]
fn check_subcommand_works_same_as_default() {
    let output = depup()
        .arg("check")
        .arg(&fixture_dir("multi-module"))
        .arg("--json")
        .output()
        .expect("Failed to run depup");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).expect("Invalid JSON output");
    assert_eq!(results.len(), 2);
}

#[test]
fn outdated_filter_excludes_current() {
    let output = depup()
        .arg(&fixture_dir("multi-module"))
        .arg("--json")
        .arg("--outdated")
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
        .arg("/nonexistent/path")
        .arg("--json")
        .output()
        .expect("Failed to run depup");

    let stdout = String::from_utf8_lossy(&output.stdout);
    // With auto-detect and a nonexistent path, it should output an empty array
    // since no ecosystem is detected (the path doesn't exist, so no pom.xml or lockfile)
    let results: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).expect("Invalid JSON output");
    assert!(results.is_empty());
}

#[test]
fn auto_detect_missing_project_returns_zero_exit() {
    let output = depup()
        .arg("/nonexistent/path")
        .output()
        .expect("Failed to run depup");

    // With a nonexistent path, no ecosystem is detected, so it prints
    // "No supported project found." and returns Ok (exit 0)
    assert!(output.status.success());
}

#[test]
fn json_output_includes_artifact() {
    let output = depup()
        .arg(&fixture_dir("multi-module"))
        .arg("--json")
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
fn update_stub_returns_not_implemented_json() {
    let output = depup()
        .arg("update")
        .arg("--json")
        .output()
        .expect("Failed to run depup");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let envelope: serde_json::Value = serde_json::from_str(&stdout).expect("Invalid JSON output");
    assert_eq!(envelope["error"]["code"], "NOT_IMPLEMENTED");
}

#[test]
fn json_output_includes_ecosystem() {
    let output = depup()
        .arg(&fixture_dir("multi-module"))
        .arg("--json")
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
