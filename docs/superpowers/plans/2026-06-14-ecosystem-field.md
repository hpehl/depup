# Ecosystem Field Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an `Ecosystem` enum to `CheckResult` so output clearly distinguishes which ecosystem (Maven vs pnpm) produced each result, with grouped section headers in table output and an `ecosystem` field in JSON.

**Architecture:** Add `Ecosystem` enum to `src/registry.rs` alongside `CheckerKind`. Thread it through `CheckResult` construction in `check.rs` and `pnpm/mod.rs`. Update `output.rs` to group results by ecosystem with section headers. Update `json.rs` to include the ecosystem field.

**Tech Stack:** Rust, serde, console crate

---

### Task 1: Add `Ecosystem` enum to `registry.rs`

**Files:**
- Modify: `src/registry.rs`

- [ ] **Step 1: Write the failing test**

Add a test in `src/registry.rs` that verifies `Ecosystem` display and default behavior:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecosystem_display() {
        assert_eq!(Ecosystem::Maven.to_string(), "Maven");
        assert_eq!(Ecosystem::Pnpm.to_string(), "pnpm");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test registry::tests::ecosystem_display`
Expected: FAIL — `Ecosystem` not found.

- [ ] **Step 3: Add `Ecosystem` enum and update `CheckResult`**

In `src/registry.rs`, add above the `CheckerKind` enum:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Ecosystem {
    Maven,
    Pnpm,
}

impl std::fmt::Display for Ecosystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Maven => write!(f, "Maven"),
            Self::Pnpm => write!(f, "pnpm"),
        }
    }
}
```

Add the `ecosystem` field to `CheckResult`:

```rust
pub struct CheckResult {
    pub ecosystem: Ecosystem,
    pub property_name: String,
    // ... rest unchanged
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test registry::tests::ecosystem_display`
Expected: PASS

- [ ] **Step 5: Fix all compilation errors**

The new `ecosystem` field will cause errors everywhere `CheckResult` is constructed. Fix each site:

**`src/command/check.rs`** — In `maven_check_all`, all three match arms construct `CheckResult` on error. Add `ecosystem: Ecosystem::Maven` to each. Import `Ecosystem` at the top.

**`src/pnpm/mod.rs`** — In `check_project`, the `map` closure constructs `CheckResult`. Add `ecosystem: Ecosystem::Pnpm`. Import `Ecosystem` at the top.

**`src/command/check.rs`** — In `pnpm_check_all`, the progress-feedback `CheckResult` instances (4 total: outdated summary, current summary, and 2 error results). Add `ecosystem: Ecosystem::Pnpm` to all four.

**`src/output.rs`** — In test helper `CheckResult` instances, add `ecosystem: Ecosystem::Maven` (or whichever is appropriate for that test).

Run: `cargo build`
Expected: Compiles with no errors.

- [ ] **Step 6: Commit**

```bash
git add src/registry.rs src/command/check.rs src/pnpm/mod.rs src/output.rs
git commit -m "feat: add Ecosystem enum to CheckResult"
```

---

### Task 2: Add `ecosystem` field to JSON output

**Files:**
- Modify: `src/json.rs`
- Modify: `src/output.rs`

- [ ] **Step 1: Write the failing test**

In `src/output.rs`, update the existing `json_output_structure` test to assert the ecosystem field:

```rust
#[test]
fn json_output_includes_ecosystem() {
    let results = vec![CheckResult {
        ecosystem: Ecosystem::Maven,
        property_name: "version.junit".to_string(),
        current_version: "5.10.0".to_string(),
        latest_version: Some("5.12.0".to_string()),
        outdated: true,
        skipped: false,
        error: None,
        artifact: Some("org.junit.jupiter:junit-jupiter".to_string()),
        kind: CheckerKind::Dependency,
    }];

    let json_results: Vec<JsonResult> = results
        .iter()
        .map(|r| JsonResult {
            ecosystem: r.ecosystem.to_string().to_lowercase(),
            property: r.property_name.clone(),
            current: r.current_version.clone(),
            latest: r.latest_version.clone(),
            status: "outdated".to_string(),
            kind: r.kind.to_string().to_lowercase(),
            error: r.error.clone(),
            artifact: r.artifact.clone(),
        })
        .collect();

    let json_str = serde_json::to_string(&json_results).unwrap();
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed[0]["ecosystem"], "maven");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test output::tests::json_output_includes_ecosystem`
Expected: FAIL — `JsonResult` has no `ecosystem` field.

- [ ] **Step 3: Add `ecosystem` field to `JsonResult`**

In `src/json.rs`, add the field as the first field:

```rust
#[derive(Debug, Serialize)]
pub struct JsonResult {
    pub ecosystem: String,
    pub property: String,
    pub current: String,
    pub latest: Option<String>,
    pub status: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact: Option<String>,
}
```

In `src/output.rs`, update `print_json` to populate the ecosystem field:

```rust
let json_results: Vec<JsonResult> = results
    .iter()
    .map(|r| {
        let status = status_label(r);
        JsonResult {
            ecosystem: r.ecosystem.to_string().to_lowercase(),
            property: r.property_name.clone(),
            // ... rest unchanged
        }
    })
    .collect();
```

Also update the existing `json_output_structure` test to include `ecosystem`:

```rust
// In the map closure:
JsonResult {
    ecosystem: r.ecosystem.to_string().to_lowercase(),
    property: r.property_name.clone(),
    // ... rest unchanged
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test output::tests`
Expected: All PASS.

- [ ] **Step 5: Commit**

```bash
git add src/json.rs src/output.rs
git commit -m "feat: include ecosystem field in JSON output"
```

---

### Task 3: Group table output by ecosystem with section headers

**Files:**
- Modify: `src/output.rs`
- Modify: `src/command/check.rs`

- [ ] **Step 1: Write the failing test**

In `src/output.rs`, add a test for the new `print_table` function that groups by ecosystem:

```rust
#[test]
fn print_table_groups_by_ecosystem() {
    let results = vec![
        CheckResult {
            ecosystem: Ecosystem::Maven,
            property_name: "version.junit".to_string(),
            current_version: "5.10.0".to_string(),
            latest_version: Some("5.12.0".to_string()),
            outdated: true,
            skipped: false,
            error: None,
            artifact: Some("org.junit.jupiter:junit-jupiter".to_string()),
            kind: CheckerKind::Dependency,
        },
        CheckResult {
            ecosystem: Ecosystem::Pnpm,
            property_name: "react".to_string(),
            current_version: "18.0.0".to_string(),
            latest_version: Some("19.0.0".to_string()),
            outdated: true,
            skipped: false,
            error: None,
            artifact: Some("react".to_string()),
            kind: CheckerKind::Pnpm,
        },
    ];

    let ecosystems: Vec<Ecosystem> = results.iter().map(|r| r.ecosystem).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
    assert_eq!(ecosystems, vec![Ecosystem::Maven, Ecosystem::Pnpm]);
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test output::tests::print_table_groups_by_ecosystem`
Expected: PASS (this test just validates grouping logic, not visual output).

- [ ] **Step 3: Add `print_ecosystem_header` and update `print_summary`**

In `src/output.rs`, add a function to print section headers and update `print_summary` to group results:

```rust
use crate::registry::{CheckResult, CheckerKind, Ecosystem};

fn print_ecosystem_header(ecosystem: Ecosystem) {
    let label = ecosystem.to_string();
    let line = "\u{2500}".repeat(3);
    println!("{} {} {}", style(line.clone()).dim(), style(label).bold(), style(line).dim());
}

pub fn print_results(results: &[CheckResult]) {
    let mut ecosystems: Vec<Ecosystem> = results
        .iter()
        .map(|r| r.ecosystem)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();

    let multiple = ecosystems.len() > 1;
    for ecosystem in &ecosystems {
        let group: Vec<&CheckResult> = results.iter().filter(|r| r.ecosystem == *ecosystem).collect();
        if multiple {
            print_ecosystem_header(*ecosystem);
        }
        print_summary(&group);
    }
}

pub fn print_summary(results: &[&CheckResult]) {
    // Same logic as before but takes &[&CheckResult]
    let total = results.len();
    let outdated = results.iter().filter(|r| r.outdated).count();
    let skipped = results.iter().filter(|r| r.skipped).count();
    let errors = results.iter().filter(|r| r.error.is_some()).count();
    let current = total - outdated - skipped - errors;

    print!("{total} properties checked: ");
    print!("{}", style(format!("{current} current")).green());
    if outdated > 0 {
        print!(", {}", style(format!("{outdated} outdated")).yellow());
    }
    if skipped > 0 {
        print!(", {}", style(format!("{skipped} skipped")).dim());
    }
    if errors > 0 {
        print!(", {}", style(format!("{errors} errors")).red());
    }

    let mut kinds: Vec<CheckerKind> = results.iter().map(|r| r.kind).collect();
    kinds.sort();
    kinds.dedup();
    let legend: Vec<String> = kinds
        .iter()
        .map(|k| format!("{} {k}", k.color().apply_to(k.symbol())))
        .collect();
    println!("  ({})", legend.join(", "));
}
```

- [ ] **Step 4: Update `check.rs` to call `print_results`**

In `src/command/check.rs`, change the non-JSON branch from:

```rust
output::print_summary(&filtered);
```

to:

```rust
output::print_results(&filtered);
```

- [ ] **Step 5: Run all tests**

Run: `cargo test`
Expected: All PASS.

- [ ] **Step 6: Commit**

```bash
git add src/output.rs src/command/check.rs
git commit -m "feat: group check output by ecosystem with section headers"
```

---

### Task 4: Update CLI integration tests

**Files:**
- Modify: `tests/cli_test.rs`

- [ ] **Step 1: Update existing JSON tests to assert ecosystem field**

In `tests/cli_test.rs`, update `json_output_includes_artifact` to also check for ecosystem:

```rust
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
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test cli_test`
Expected: All PASS.

- [ ] **Step 3: Commit**

```bash
git add tests/cli_test.rs
git commit -m "test: verify ecosystem field in CLI JSON output"
```
