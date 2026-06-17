//! Terminal and JSON output formatting.
//!
//! All subcommands share the same output functions:
//! - [`print_table`] groups results by ecosystem/kind and prints styled lines.
//! - [`print_json`] serializes any `Serialize` value as pretty JSON.
//!
//! Each result type implements [`OutputLine`] to provide its version label
//! and styled status column, while the shared columns (artifact, source)
//! are formatted uniformly via the [`DependencyInfo`] trait.

mod format;
pub mod json;
mod line;
mod summary;

pub use summary::{audit_summary, check_summary, update_summary};

use console::style;
use serde::Serialize;

use format::print_grouped;
use line::OutputLine;

/// Prints a styled table grouped by ecosystem and kind, with a summary line.
///
/// The `empty_msg` is shown when `results` is empty (pass `""` to print nothing).
/// The `summary` closure prints subcommand-specific summary statistics.
pub fn print_table<T: OutputLine>(results: &[T], empty_msg: &str, summary: impl Fn(&[T])) {
    if results.is_empty() {
        if !empty_msg.is_empty() {
            println!("{}", style(empty_msg).dim());
        }
        return;
    }

    print_grouped(results);
    summary(results);
}

/// Prints any serializable value as pretty JSON.
pub fn print_json(items: &impl Serialize) {
    println!(
        "{}",
        serde_json::to_string_pretty(items).unwrap_or_else(|_| "[]".to_string())
    );
}

#[cfg(test)]
mod tests {
    use crate::model::{CheckResult, CommandResult, Dependency, DependencyKind, Ecosystem};

    #[test]
    fn print_table_groups_by_ecosystem() {
        let results = vec![
            CheckResult::checked(
                Dependency::new(
                    Ecosystem::Maven,
                    DependencyKind::Dependency,
                    "org.junit.jupiter:junit-jupiter".to_string(),
                    Some("version.junit".to_string()),
                    "pom.xml".to_string(),
                ),
                "5.10.0".to_string(),
                "5.12.0".to_string(),
                true,
            ),
            CheckResult::checked(
                Dependency::new(
                    Ecosystem::Npm,
                    DependencyKind::NpmDep,
                    "react".to_string(),
                    None,
                    "package.json".to_string(),
                ),
                "18.0.0".to_string(),
                "19.0.0".to_string(),
                true,
            ),
        ];

        let ecosystems: Vec<Ecosystem> = results
            .iter()
            .map(|r| r.ecosystem())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        assert_eq!(ecosystems, vec![Ecosystem::Maven, Ecosystem::Npm]);
    }
}
