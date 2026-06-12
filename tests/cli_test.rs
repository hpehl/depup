use std::path::PathBuf;
use std::process::Command;

fn fixture_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn mvnup() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mvnup"))
}

#[test]
fn json_output_returns_array() {
    let output = mvnup()
        .arg(&fixture_dir("multi-module"))
        .arg("--json")
        .output()
        .expect("Failed to run mvnup");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).expect("Invalid JSON output");
    assert_eq!(results.len(), 2);
}

#[test]
fn check_subcommand_works_same_as_default() {
    let output = mvnup()
        .arg("check")
        .arg(&fixture_dir("multi-module"))
        .arg("--json")
        .output()
        .expect("Failed to run mvnup");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).expect("Invalid JSON output");
    assert_eq!(results.len(), 2);
}

#[test]
fn outdated_filter_excludes_current() {
    let output = mvnup()
        .arg(&fixture_dir("multi-module"))
        .arg("--json")
        .arg("--outdated")
        .output()
        .expect("Failed to run mvnup");

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
    let output = mvnup()
        .arg("/nonexistent/path")
        .arg("--json")
        .output()
        .expect("Failed to run mvnup");

    assert!(!output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let envelope: serde_json::Value =
        serde_json::from_str(&stdout).expect("Invalid JSON error output");
    assert_eq!(envelope["error"]["code"], "POM_NOT_FOUND");
    assert!(
        envelope["error"]["message"]
            .as_str()
            .unwrap()
            .contains("nonexistent")
    );
}

#[test]
fn missing_pom_returns_nonzero_exit() {
    let output = mvnup()
        .arg("/nonexistent/path")
        .output()
        .expect("Failed to run mvnup");

    assert!(!output.status.success());
}

#[test]
fn json_output_includes_artifact() {
    let output = mvnup()
        .arg(&fixture_dir("multi-module"))
        .arg("--json")
        .output()
        .expect("Failed to run mvnup");

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
