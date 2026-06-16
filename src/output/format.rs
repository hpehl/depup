use std::collections::BTreeSet;

use console::style;

use crate::model::{DependencyKind, Ecosystem, Severity};

use super::line::OutputLine;

pub(super) const ARTIFACT_WIDTH: usize = 40;
pub(super) const VERSION_WIDTH: usize = 30;
pub(super) const SOURCE_WIDTH: usize = 25;

pub(super) fn format_columns(
    item: &impl OutputLine,
) -> (console::StyledObject<String>, String, String) {
    let artifact = truncate_middle_pad(item.artifact(), ARTIFACT_WIDTH);
    let styled_artifact = kind_color(item.kind()).apply_to(artifact);
    let version = format_version_column(&item.version_value(), item.property(), VERSION_WIDTH);
    let source = truncate_middle_pad(item.source(), SOURCE_WIDTH);
    (styled_artifact, version, source)
}

/// Formats the version column with the version right-aligned when a property is present.
fn format_version_column(version: &str, property: Option<&str>, width: usize) -> String {
    match property {
        Some(prop) => {
            let ver_part = format!("({version})");
            let ver_len = ver_part.chars().count();
            if ver_len >= width {
                truncate_middle_pad(&ver_part, width)
            } else {
                let prop_width = width - ver_len - 1;
                let prop_col = truncate_middle_pad(prop, prop_width);
                format!("{prop_col} {ver_part}")
            }
        }
        None => truncate_middle_pad(version, width),
    }
}

pub(super) fn truncate_middle_pad(s: &str, width: usize) -> String {
    let char_count = s.chars().count();
    if char_count > width {
        let ellipsis = "\u{2026}";
        let half = (width - 1) / 2;
        let remainder = width - 1 - half;
        let prefix: String = s.chars().take(half).collect();
        let suffix: String = s.chars().skip(char_count - remainder).collect();
        format!("{prefix}{ellipsis}{suffix}")
    } else {
        format!("{s:<width$}")
    }
}

pub(super) fn kind_color(kind: DependencyKind) -> console::Style {
    match kind {
        DependencyKind::Dependency => console::Style::new().cyan(),
        DependencyKind::Plugin => console::Style::new().magenta(),
        DependencyKind::Tool => console::Style::new().green(),
        DependencyKind::NpmDep | DependencyKind::NpmDevDep => console::Style::new().blue(),
    }
}

pub(super) fn kind_symbol(kind: DependencyKind) -> &'static str {
    match kind {
        DependencyKind::Dependency => "\u{25cf}",
        DependencyKind::Plugin => "\u{25a0}",
        DependencyKind::Tool => "\u{25b2}",
        DependencyKind::NpmDep | DependencyKind::NpmDevDep => "\u{25c6}",
    }
}

pub(super) fn kind_group_label(kind: DependencyKind) -> &'static str {
    match kind {
        DependencyKind::Dependency => "Dependencies",
        DependencyKind::Plugin => "Plugins",
        DependencyKind::Tool => "Tool Versions",
        DependencyKind::NpmDep => "Dependencies",
        DependencyKind::NpmDevDep => "Dev Dependencies",
    }
}

pub(super) fn print_kind_legend(kinds: &[DependencyKind]) {
    let mut sorted = kinds.to_vec();
    sorted.sort();
    sorted.dedup();
    let legend: Vec<String> = sorted
        .iter()
        .map(|k| format!("{} {k}", kind_color(*k).apply_to(kind_symbol(*k))))
        .collect();
    println!("  ({})", legend.join(", "));
}

pub(super) fn severity_style(severity: Severity) -> console::Style {
    match severity {
        Severity::Critical => console::Style::new().red().bold(),
        Severity::High => console::Style::new().red(),
        Severity::Medium => console::Style::new().yellow(),
        Severity::Low => console::Style::new().dim(),
        Severity::Unknown => console::Style::new().dim(),
    }
}

pub(super) fn print_ecosystem_header(ecosystem: Ecosystem) {
    let label = ecosystem.to_string();
    let line = "\u{2500}".repeat(3);
    println!(
        "{} {} {}",
        style(line.clone()).dim(),
        style(label).bold(),
        style(line).dim()
    );
}

pub(super) fn print_grouped<T: OutputLine>(items: &[T]) {
    let ecosystems: Vec<Ecosystem> = items
        .iter()
        .map(|r| r.ecosystem())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let multiple_ecosystems = ecosystems.len() > 1;
    for ecosystem in &ecosystems {
        let mut group: Vec<&T> = items
            .iter()
            .filter(|r| r.ecosystem() == *ecosystem)
            .collect();
        group.sort_by(|a, b| {
            a.kind()
                .cmp(&b.kind())
                .then_with(|| a.artifact().cmp(b.artifact()))
        });

        if multiple_ecosystems {
            print_ecosystem_header(*ecosystem);
        }

        let kinds: Vec<DependencyKind> = group
            .iter()
            .map(|r| r.kind())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let multiple_kinds = kinds.len() > 1;

        for kind in &kinds {
            if multiple_kinds {
                println!("  {}", style(kind_group_label(*kind)).dim().bold());
            }
            for item in group.iter().filter(|r| r.kind() == *kind) {
                item.print_line();
            }
        }
        println!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_column_with_property_right_aligns() {
        let result = format_version_column("5.10.0", Some("version.junit"), 30);
        assert_eq!(result.chars().count(), 30);
        assert!(result.ends_with("(5.10.0)"));
        assert!(result.starts_with("version.junit"));
    }

    #[test]
    fn version_column_without_property_left_aligns() {
        let result = format_version_column("33.0.0-jre", None, 30);
        assert_eq!(result, format!("{:<30}", "33.0.0-jre"));
    }

    #[test]
    fn version_column_truncates_long_property() {
        let result = format_version_column("1.0.0", Some("version.very.long.property.name"), 30);
        assert_eq!(result.chars().count(), 30);
        assert!(result.ends_with("(1.0.0)"));
        assert!(result.contains('\u{2026}'));
    }

    #[test]
    fn truncate_short_string_pads() {
        let result = truncate_middle_pad("hello", 10);
        assert_eq!(result, "hello     ");
        assert_eq!(result.len(), 10);
    }

    #[test]
    fn truncate_exact_width_no_change() {
        let result = truncate_middle_pad("abcde", 5);
        assert_eq!(result, "abcde");
    }

    #[test]
    fn truncate_long_string_inserts_ellipsis() {
        let result = truncate_middle_pad("abcdefghij", 7);
        assert!(result.contains('\u{2026}'));
        assert_eq!(result.chars().count(), 7);
        assert!(result.starts_with("abc"));
        assert!(result.ends_with("hij"));
    }

    #[test]
    fn truncate_multibyte_does_not_panic() {
        let result = truncate_middle_pad("ä\u{f6}ü\u{e9}\u{e8}\u{ea}\u{eb}\u{e0}", 5);
        assert_eq!(result.chars().count(), 5);
    }
}
