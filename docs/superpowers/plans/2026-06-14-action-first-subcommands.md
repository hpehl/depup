# Action-First Subcommand Refactoring

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restructure the CLI from ecosystem-first (`maven check`, `pnpm check`) to action-first (`check`, `update`, `audit`) subcommands that run across all detected ecosystems in a single invocation.

**Architecture:** The `check` subcommand discovers every ecosystem present in the target path (Maven if `pom.xml` exists, pnpm if lockfile/packageManager exists) and runs all of them, merging results. The `maven` and `pnpm` top-level subcommands are removed entirely. `update` and `audit` are added as stubs. Each action lives in its own file under `src/command/`. The ecosystem-specific check logic stays in `maven/` and `pnpm/` modules — the command layer orchestrates discovery and result aggregation.

**Tech Stack:** Rust, clap (builder API), tokio, indicatif

---

## Current → New CLI Structure

```
BEFORE:                                    AFTER:
depup [path] [--json] [--outdated]         depup check [path] [--json] [--outdated] [--include-pre-releases]
depup check [path]                         depup update [path] [--json]              (stub)
depup maven check [path]                   depup audit [path] [--json]               (stub)
depup pnpm check [path]                    depup completions [shell] [-i]
depup completions [shell] [-i]
```

The default (no subcommand) still runs `check` with auto-detection.

## File Map

| File | Action | Responsibility |
|---|---|---|
| `src/app.rs` | **Modify** | Remove `maven`/`pnpm` subcommands, add `update`/`audit` stubs, restructure `check` args |
| `src/main.rs` | **Modify** | Remove `maven`/`pnpm` dispatch branches, add `update`/`audit` dispatch |
| `src/command/mod.rs` | **Modify** | Add `pub mod update; pub mod audit;` |
| `src/command/check.rs` | **Modify** | Replace `auto_check`/`maven_check`/`pnpm_check` public API with single `check()` that runs all ecosystems |
| `src/command/update.rs` | **Create** | Stub for `update` subcommand |
| `src/command/audit.rs` | **Create** | Stub for `audit` subcommand |
| `tests/cli_test.rs` | **Modify** | Remove `maven_check_subcommand_works` and `maven_missing_pom_returns_json_error` tests, add multi-ecosystem test |
| `CLAUDE.md` | **Modify** | Update CLI examples and architecture docs |

---

### Task 1: Restructure `app.rs` — new subcommand tree

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Write the failing test**

There are no unit tests in `app.rs` — the CLI structure is tested via integration tests in `tests/cli_test.rs`. We'll verify via compilation and existing CLI tests after each change. First, confirm current tests pass:

Run: `cargo test --test cli_test 2>&1 | tail -5`
Expected: all tests pass

- [ ] **Step 2: Rewrite `build_app()` to remove `maven`/`pnpm` subcommands and add `update`/`audit` stubs**

Replace the entire content of `src/app.rs` with:

```rust
use clap::builder::Styles;
use clap::builder::styling::{AnsiColor, Effects};
use clap::{Arg, ArgAction, Command, crate_name, crate_version};

pub fn build_app() -> Command {
    let styles = Styles::styled()
        .header(AnsiColor::Green.on_default() | Effects::BOLD)
        .usage(AnsiColor::Green.on_default() | Effects::BOLD)
        .literal(AnsiColor::Blue.on_default() | Effects::BOLD)
        .placeholder(AnsiColor::Cyan.on_default());

    let app = Command::new(crate_name!())
        .version(crate_version!())
        .about("Check dependency versions across multiple ecosystems")
        .styles(styles)
        .propagate_version(true)
        .arg(
            Arg::new("json")
                .long("json")
                .global(true)
                .action(ArgAction::SetTrue)
                .help("Output results as JSON (for machine consumption)"),
        )
        .subcommand(check_args(Command::new("check")
            .about("Check dependencies for newer versions")))
        .subcommand(update_args(Command::new("update")
            .about("Update dependencies to their latest versions")))
        .subcommand(audit_args(Command::new("audit")
            .about("Audit dependencies for known vulnerabilities")))
        .subcommand(
            Command::new("completions")
                .about("Generate and install shell completions")
                .arg(
                    Arg::new("shell")
                        .help("The shell to generate completions for [default: auto-detected]"),
                )
                .arg(
                    Arg::new("install")
                        .short('i')
                        .long("install")
                        .action(ArgAction::SetTrue)
                        .help("Install completions to the standard location for the shell"),
                ),
        );
    check_args(app)
}

fn check_args(cmd: Command) -> Command {
    cmd.arg(
        Arg::new("path")
            .default_value(".")
            .help("Path to the project root (auto-detects ecosystems)"),
    )
    .arg(
        Arg::new("outdated")
            .long("outdated")
            .action(ArgAction::SetTrue)
            .help("Only show outdated dependencies"),
    )
    .arg(
        Arg::new("include-pre-releases")
            .long("include-pre-releases")
            .action(ArgAction::SetTrue)
            .help("Include pre-release versions (Maven only)"),
    )
}

fn update_args(cmd: Command) -> Command {
    cmd.arg(
        Arg::new("path")
            .default_value(".")
            .help("Path to the project root (auto-detects ecosystems)"),
    )
}

fn audit_args(cmd: Command) -> Command {
    cmd.arg(
        Arg::new("path")
            .default_value(".")
            .help("Path to the project root (auto-detects ecosystems)"),
    )
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check 2>&1 | tail -10`
Expected: Compilation errors in `main.rs` (references to removed `maven`/`pnpm` subcommands) — that's expected, we fix it in the next task.

- [ ] **Step 4: Commit**

```bash
git add src/app.rs
git commit -m "refactor: restructure CLI to action-first subcommands

Remove maven/pnpm top-level subcommands. Add check, update, audit
as top-level actions. check auto-detects all ecosystems in the path."
```

---

### Task 2: Rewrite `main.rs` dispatch

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Replace the `run()` function dispatch**

Replace the content of `src/main.rs` with:

```rust
mod app;
mod args;
mod command;
mod constants;
mod error;
mod json;
mod maven;
mod output;
mod pnpm;
mod progress;
mod registry;
mod version;

use anyhow::Result;

use crate::error::{DepupError, JsonErrorEnvelope};

#[tokio::main]
async fn main() {
    clap_complete::CompleteEnv::with_factory(app::build_app).complete();

    let json = std::env::args().any(|a| a == "--json");
    if let Err(e) = run().await {
        if json {
            let envelope = JsonErrorEnvelope::from_anyhow(&e);
            match serde_json::to_string(&envelope) {
                Ok(json) => println!("{json}"),
                Err(ser) => eprintln!("Error: {e:#}\n(JSON serialization also failed: {ser})"),
            }
        } else {
            eprintln!("Error: {e:#}");
        }
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let matches = app::build_app()
        .try_get_matches()
        .map_err(classify_clap_error)?;

    match matches.subcommand() {
        Some(("check", m)) => command::check::check(m).await,
        Some(("update", m)) => command::update::update(m).await,
        Some(("audit", m)) => command::audit::audit(m).await,
        Some(("completions", m)) => command::completions::completions(m),
        _ => command::check::check(&matches).await,
    }
}

#[allow(clippy::needless_pass_by_value)]
fn classify_clap_error(err: clap::Error) -> anyhow::Error {
    match err.kind() {
        clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
            err.exit();
        }
        _ => DepupError::clap_parse_error(err.to_string().trim()).into(),
    }
}
```

- [ ] **Step 2: Verify it compiles (will fail — `command::update` and `command::audit` don't exist yet)**

Run: `cargo check 2>&1 | tail -10`
Expected: Errors about missing `update` and `audit` modules and `check::check` function not found.

---

### Task 3: Create `update` and `audit` stubs

**Files:**
- Create: `src/command/update.rs`
- Create: `src/command/audit.rs`
- Modify: `src/command/mod.rs`

- [ ] **Step 1: Create `src/command/update.rs`**

```rust
use anyhow::Result;
use clap::ArgMatches;

use crate::args;

pub async fn update(matches: &ArgMatches) -> Result<()> {
    let path = args::path_argument(matches);
    let json = args::is_json(matches);
    let root = path.canonicalize().unwrap_or_else(|_| path.clone());

    if json {
        println!(r#"{{"error": {{"code": "NOT_IMPLEMENTED", "message": "update is not yet implemented"}}}}"#);
    } else {
        println!(
            "Update is not yet implemented. Would update dependencies in {}",
            root.display()
        );
    }
    Ok(())
}
```

- [ ] **Step 2: Create `src/command/audit.rs`**

```rust
use anyhow::Result;
use clap::ArgMatches;

use crate::args;

pub async fn audit(matches: &ArgMatches) -> Result<()> {
    let path = args::path_argument(matches);
    let json = args::is_json(matches);
    let root = path.canonicalize().unwrap_or_else(|_| path.clone());

    if json {
        println!(r#"{{"error": {{"code": "NOT_IMPLEMENTED", "message": "audit is not yet implemented"}}}}"#);
    } else {
        println!(
            "Audit is not yet implemented. Would audit dependencies in {}",
            root.display()
        );
    }
    Ok(())
}
```

- [ ] **Step 3: Update `src/command/mod.rs`**

```rust
pub mod audit;
pub mod check;
pub mod completions;
pub mod update;
```

- [ ] **Step 4: Verify compilation (will still fail — `check::check` doesn't exist yet)**

Run: `cargo check 2>&1 | tail -5`
Expected: Error about `check::check` not found. The stubs compile fine.

- [ ] **Step 5: Commit**

```bash
git add src/command/update.rs src/command/audit.rs src/command/mod.rs
git commit -m "feat: add update and audit subcommand stubs"
```

---

### Task 4: Rewrite `check.rs` — unified multi-ecosystem check

This is the core change. The new `check()` function discovers all ecosystems present in the path and runs them all, merging results.

**Files:**
- Modify: `src/command/check.rs`

- [ ] **Step 1: Replace `src/command/check.rs` with the unified implementation**

The key change: `auto_check` is replaced by `check()` which discovers Maven AND pnpm independently, runs both if present, and merges results. `maven_check` and `pnpm_check` become private helpers that return `Vec<CheckResult>` instead of printing directly.

```rust
use std::sync::Arc;

use anyhow::Result;
use clap::ArgMatches;
use indicatif::MultiProgress;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::Instant;

use crate::args;
use crate::constants::MAX_CONCURRENT_REQUESTS;
use crate::maven::discovery::{self, ArtifactMapping, VersionProperty};
use crate::maven::node::NodeChecker;
use crate::maven::npm::NpmChecker;
use crate::maven::pom::ArtifactKind;
use crate::maven::registry::MavenChecker;
use crate::output;
use crate::progress::{self, Progress};
use crate::registry::{CheckResult, CheckerKind};

pub async fn check(matches: &ArgMatches) -> Result<()> {
    let path = args::path_argument(matches);
    let json = args::is_json(matches);
    let outdated = args::is_outdated(matches);
    let releases_only = args::releases_only(matches);

    let instant = Instant::now();
    let root = path.canonicalize().unwrap_or_else(|_| path.clone());

    let has_maven = root.join("pom.xml").exists();
    let has_pnpm = has_pnpm_signals(&root);

    if !has_maven && !has_pnpm {
        if json {
            println!("[]");
        } else {
            println!(
                "No supported project found in {}.\n\
                 Expected pom.xml (Maven) or pnpm-lock.yaml/package.json (pnpm).",
                root.display()
            );
        }
        return Ok(());
    }

    let mut all_results: Vec<CheckResult> = Vec::new();

    if has_maven {
        let results = maven_check(&root, json, releases_only).await?;
        all_results.extend(results);
    }

    if has_pnpm {
        let results = pnpm_check(&root, json).await?;
        all_results.extend(results);
    }

    if outdated {
        all_results.retain(|r| r.outdated);
    }

    if json {
        output::print_json(&all_results);
    } else {
        println!();
        output::print_summary(&all_results);
        progress::done(instant);
    }

    let has_outdated = all_results.iter().any(|r| r.outdated);
    if has_outdated {
        std::process::exit(1);
    }

    Ok(())
}

fn has_pnpm_signals(root: &std::path::Path) -> bool {
    if root.join("pnpm-lock.yaml").exists() {
        return true;
    }
    if root.join("package.json").exists()
        && let Ok(content) = std::fs::read_to_string(root.join("package.json"))
        && content.contains("\"pnpm@")
    {
        return true;
    }
    false
}

// ------------------------------------------------------------------
// Maven
// ------------------------------------------------------------------

enum MavenCheckTask {
    Maven {
        mapping: ArtifactMapping,
        checker: Arc<MavenChecker>,
    },
    Node {
        property: VersionProperty,
        checker: Arc<NodeChecker>,
    },
    Npm {
        property: VersionProperty,
        package: &'static str,
        checker: Arc<NpmChecker>,
    },
}

impl MavenCheckTask {
    fn kind(&self) -> CheckerKind {
        match self {
            Self::Maven { mapping, .. } => match mapping.kind {
                ArtifactKind::Dependency => CheckerKind::Dependency,
                ArtifactKind::Plugin => CheckerKind::Plugin,
            },
            Self::Node { .. } => CheckerKind::Node,
            Self::Npm { .. } => CheckerKind::Npm,
        }
    }

    fn property_name(&self) -> &str {
        match self {
            Self::Maven { mapping, .. } => &mapping.property.name,
            Self::Node { property, .. } | Self::Npm { property, .. } => &property.name,
        }
    }

    fn current_value(&self) -> &str {
        match self {
            Self::Maven { mapping, .. } => &mapping.property.current_value,
            Self::Node { property, .. } | Self::Npm { property, .. } => &property.current_value,
        }
    }

    fn artifact_label(&self) -> String {
        match self {
            Self::Maven { mapping, .. } => {
                format!("{}:{}", mapping.group_id, mapping.artifact_id)
            }
            Self::Node { .. } => "nodejs.org".to_string(),
            Self::Npm { package, .. } => (*package).to_string(),
        }
    }
}

async fn maven_check(
    root: &std::path::Path,
    json: bool,
    releases_only: bool,
) -> Result<Vec<CheckResult>> {
    if !json {
        progress::step("\u{1f50d}", "Discovering POM modules...");
    }
    let discovery_result = discovery::discover(root)?;

    let maven_checker = Arc::new(MavenChecker::new(
        releases_only,
        discovery_result.repositories,
    ));
    let node_checker = Arc::new(NodeChecker::new(releases_only));
    let npm_checker = Arc::new(NpmChecker::new(releases_only));

    let mut tasks: Vec<MavenCheckTask> = discovery_result
        .mappings
        .into_iter()
        .map(|mapping| MavenCheckTask::Maven {
            mapping,
            checker: Arc::clone(&maven_checker),
        })
        .collect();

    for property in discovery_result.orphan_properties {
        if NodeChecker::matches(&property.name) {
            tasks.push(MavenCheckTask::Node {
                property,
                checker: Arc::clone(&node_checker),
            });
        } else if let Some(package) = NpmChecker::matches(&property.name) {
            tasks.push(MavenCheckTask::Npm {
                property,
                package,
                checker: Arc::clone(&npm_checker),
            });
        }
    }

    if tasks.is_empty() {
        if !json {
            println!("No Maven version properties found.");
        }
        return Ok(Vec::new());
    }

    if !json {
        progress::step(
            "\u{2699}\u{fe0f}",
            &format!("Checking {} Maven properties...", tasks.len()),
        );
    }

    let results = maven_check_all(tasks, json).await;
    Ok(results)
}

async fn maven_check_all(tasks: Vec<MavenCheckTask>, json: bool) -> Vec<CheckResult> {
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS));
    let multi_progress = MultiProgress::new();
    let mut join_set = JoinSet::new();

    for task in tasks {
        let semaphore = Arc::clone(&semaphore);
        let kind = task.kind();
        let property_name = task.property_name().to_string();
        let current_value = task.current_value().to_string();
        let artifact_label = task.artifact_label();

        let progress = if json {
            Progress::hidden(kind, &property_name, &artifact_label, &current_value)
        } else {
            Progress::join(
                &multi_progress,
                kind,
                &property_name,
                &artifact_label,
                &current_value,
            )
        };

        join_set.spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            let result = match task {
                MavenCheckTask::Maven {
                    ref mapping,
                    ref checker,
                } => checker.check(mapping).await.unwrap_or_else(|e| CheckResult {
                    property_name: mapping.property.name.clone(),
                    current_version: mapping.property.current_value.clone(),
                    latest_version: None,
                    outdated: false,
                    skipped: false,
                    error: Some(e.to_string()),
                    artifact: Some(format!("{}:{}", mapping.group_id, mapping.artifact_id)),
                    kind,
                }),
                MavenCheckTask::Node {
                    ref property,
                    ref checker,
                } => checker.check(property).await.unwrap_or_else(|e| CheckResult {
                    property_name: property.name.clone(),
                    current_version: property.current_value.clone(),
                    latest_version: None,
                    outdated: false,
                    skipped: false,
                    error: Some(e.to_string()),
                    artifact: Some("nodejs.org".to_string()),
                    kind,
                }),
                MavenCheckTask::Npm {
                    ref property,
                    package,
                    ref checker,
                } => checker
                    .check(property, package)
                    .await
                    .unwrap_or_else(|e| CheckResult {
                        property_name: property.name.clone(),
                        current_version: property.current_value.clone(),
                        latest_version: None,
                        outdated: false,
                        skipped: false,
                        error: Some(e.to_string()),
                        artifact: Some(package.to_string()),
                        kind,
                    }),
            };
            progress.finish_with_result(&result);
            result
        });
    }

    join_set.join_all().await
}

// ------------------------------------------------------------------
// pnpm
// ------------------------------------------------------------------

async fn pnpm_check(root: &std::path::Path, json: bool) -> Result<Vec<CheckResult>> {
    if !json {
        progress::step("\u{1f50d}", "Discovering pnpm projects...");
    }
    let projects = crate::pnpm::discovery::discover(root);

    if projects.is_empty() {
        if !json {
            println!("No pnpm projects found.");
        }
        return Ok(Vec::new());
    }

    if !json {
        progress::step(
            "\u{2699}\u{fe0f}",
            &format!("Checking {} pnpm project(s)...", projects.len()),
        );
    }

    let results = pnpm_check_all(&projects, json).await;
    Ok(results)
}

async fn pnpm_check_all(
    projects: &[crate::pnpm::discovery::PnpmProject],
    json: bool,
) -> Vec<CheckResult> {
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS));
    let multi_progress = MultiProgress::new();
    let mut join_set = JoinSet::new();

    for project in projects {
        let semaphore = Arc::clone(&semaphore);
        let project_path = project.path.clone();
        let project_name = project.name.clone();

        let progress = if json {
            Progress::hidden(CheckerKind::Pnpm, &project_name, "", "")
        } else {
            Progress::join(
                &multi_progress,
                CheckerKind::Pnpm,
                &project_name,
                &project_path.display().to_string(),
                "",
            )
        };

        join_set.spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            let results = crate::pnpm::check_project(&project_path).await;
            match results {
                Ok(check_results) => {
                    let count = check_results.len();
                    let outdated_count = check_results.iter().filter(|r| r.outdated).count();
                    if outdated_count > 0 {
                        progress.finish_with_result(&CheckResult {
                            property_name: project_name,
                            current_version: String::new(),
                            latest_version: None,
                            outdated: true,
                            skipped: false,
                            error: None,
                            artifact: Some(format!("{outdated_count}/{count} outdated")),
                            kind: CheckerKind::Pnpm,
                        });
                    } else {
                        progress.finish_with_result(&CheckResult {
                            property_name: project_name,
                            current_version: String::new(),
                            latest_version: None,
                            outdated: false,
                            skipped: false,
                            error: None,
                            artifact: Some(format!("{count} packages")),
                            kind: CheckerKind::Pnpm,
                        });
                    }
                    check_results
                }
                Err(e) => {
                    progress.finish_with_result(&CheckResult {
                        property_name: project_name.clone(),
                        current_version: String::new(),
                        latest_version: None,
                        outdated: false,
                        skipped: false,
                        error: Some(e.to_string()),
                        artifact: None,
                        kind: CheckerKind::Pnpm,
                    });
                    vec![CheckResult {
                        property_name: project_name,
                        current_version: String::new(),
                        latest_version: None,
                        outdated: false,
                        skipped: false,
                        error: Some(e.to_string()),
                        artifact: None,
                        kind: CheckerKind::Pnpm,
                    }]
                }
            }
        });
    }

    join_set.join_all().await.into_iter().flatten().collect()
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check 2>&1 | tail -5`
Expected: Clean compilation, no errors.

- [ ] **Step 3: Run `cargo clippy`**

Run: `cargo clippy 2>&1 | tail -10`
Expected: No warnings on changed files.

- [ ] **Step 4: Commit**

```bash
git add src/command/check.rs src/main.rs
git commit -m "refactor: unify check to discover all ecosystems

check() now discovers Maven and pnpm independently in the target
path and runs both when present, merging results. Removes separate
maven_check/pnpm_check public entry points."
```

---

### Task 5: Update CLI integration tests

**Files:**
- Modify: `tests/cli_test.rs`

- [ ] **Step 1: Remove tests that reference `maven` subcommand and update remaining tests**

Replace `tests/cli_test.rs` with:

```rust
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
fn auto_detect_missing_project_returns_nonzero_exit() {
    let output = depup()
        .arg("/nonexistent/path")
        .output()
        .expect("Failed to run depup");

    assert!(!output.status.success());
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
fn update_subcommand_runs_without_error() {
    let output = depup()
        .arg("update")
        .arg(".")
        .output()
        .expect("Failed to run depup");

    assert!(output.status.success());
}

#[test]
fn audit_subcommand_runs_without_error() {
    let output = depup()
        .arg("audit")
        .arg(".")
        .output()
        .expect("Failed to run depup");

    assert!(output.status.success());
}

#[test]
fn update_json_returns_not_implemented() {
    let output = depup()
        .arg("update")
        .arg("--json")
        .arg(".")
        .output()
        .expect("Failed to run depup");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("Invalid JSON output");
    assert_eq!(parsed["error"]["code"], "NOT_IMPLEMENTED");
}

#[test]
fn audit_json_returns_not_implemented() {
    let output = depup()
        .arg("audit")
        .arg("--json")
        .arg(".")
        .output()
        .expect("Failed to run depup");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("Invalid JSON output");
    assert_eq!(parsed["error"]["code"], "NOT_IMPLEMENTED");
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo test --test cli_test 2>&1 | tail -15`
Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add tests/cli_test.rs
git commit -m "test: update CLI tests for action-first subcommands

Remove tests for removed maven/pnpm subcommands. Add tests for
update and audit stubs."
```

---

### Task 6: Run full test suite and lint

**Files:** None (verification only)

- [ ] **Step 1: Run all tests**

Run: `cargo test 2>&1 | tail -20`
Expected: All tests pass.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy 2>&1 | tail -10`
Expected: No warnings.

- [ ] **Step 3: Run formatter**

Run: `cargo fmt -- --check 2>&1`
Expected: No formatting issues.

---

### Task 7: Update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Update the Build & Test section CLI examples**

Replace the `cargo run` lines in the Build & Test section with:

```bash
cargo run -- check /path                  # auto-detect ecosystems and check all
cargo run -- check --json /path           # check with JSON output
cargo run -- check --outdated /path       # only show outdated dependencies
cargo run -- update /path                 # update dependencies (stub)
cargo run -- audit /path                  # audit dependencies (stub)
cargo run -- completions                  # generate shell completions
```

- [ ] **Step 2: Update the CLI Layer documentation**

Update the `app.rs` bullet to reflect the new subcommand structure:

```
- **`app.rs`** — Defines the clap `Command` tree using the builder API (not derive macros). Subcommands: `check` (auto-detects all ecosystems), `update` (stub), `audit` (stub), `completions`. Global `--json` flag. Styled help text. Separated from `main.rs` so the completion system can build the command tree independently.
```

Update the `main.rs` bullet:

```
- **`main.rs`** — Entry point. Wires `CompleteEnv` for dynamic shell completions, dispatches subcommands (`check`, `update`, `audit`, `completions`), handles top-level error reporting with JSON error envelope support.
```

- [ ] **Step 3: Update the Command Layer documentation**

Replace the `check.rs` bullet:

```
- **`check.rs`** — Orchestrates check pipelines across all ecosystems:
  - `check()` — Discovers all ecosystems in the target path (Maven if `pom.xml` exists, pnpm if lockfile exists), runs all found, merges results.
  - Maven path: discover POM modules, check versions concurrently with progress spinners, sort/filter results.
  - pnpm path: discover pnpm projects, run `pnpm outdated --format json` on each, aggregate results.
  - Both use `tokio::task::JoinSet` for parallel checks with semaphore-based rate limiting.
  - Output: combined table or JSON across all ecosystems.

- **`update.rs`** — Stub for future `update` subcommand.

- **`audit.rs`** — Stub for future `audit` subcommand.
```

- [ ] **Step 4: Remove the ecosystem-specific `cargo run` examples that reference `maven` or `pnpm` subcommands**

Remove these lines from Build & Test:
```
cargo run -- maven check /path
cargo run -- maven check --json /path
cargo run -- pnpm check /path
cargo run -- pnpm check --json /path
```

- [ ] **Step 5: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md for action-first subcommand structure"
```
