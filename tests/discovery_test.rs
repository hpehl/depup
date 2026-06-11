use std::path::PathBuf;

// Import from the library — need to expose modules
// For now we test via the binary's modules directly
// by using the test fixture and checking discovery output

#[test]
fn discovers_multi_module_properties() {
    let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("multi-module");

    // We can't directly call discovery::discover from integration tests
    // without a library target. Let's test via the CLI binary instead.
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_mvnup"))
        .arg(&fixture_dir)
        .arg("--json")
        .output()
        .expect("Failed to run mvnup");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).expect("Invalid JSON output");

    // Should find both version properties
    assert_eq!(results.len(), 2, "Expected 2 properties, got: {stdout}");

    let props: Vec<&str> = results
        .iter()
        .map(|r| r["property"].as_str().unwrap())
        .collect();
    assert!(props.contains(&"version.compiler.plugin"));
    assert!(props.contains(&"version.junit"));

    // Both should have current versions set
    for result in &results {
        assert!(
            !result["current"].as_str().unwrap().is_empty(),
            "Current version should not be empty"
        );
    }
}
