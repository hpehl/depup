use console::style;

use crate::dependency::{AuditResult, DependencyInfo, UpdateResult, VersionResult, VersionStatus};

use super::format::{
    format_columns, format_version_with_property, severity_style, truncate_middle_pad,
};

/// Subcommand-specific line rendering. Each result type provides its own
/// version label and status output while sharing the common column layout.
pub(crate) trait OutputLine: DependencyInfo {
    fn version_label(&self) -> String;
    fn print_line(&self);
}

impl OutputLine for VersionResult {
    fn version_label(&self) -> String {
        format_version_with_property(&self.current_version, self.property())
    }

    fn print_line(&self) {
        let (styled_artifact, version, source) = format_columns(self);
        match &self.status {
            VersionStatus::UpToDate { .. } => {
                println!(
                    "  {} {}  {}  {}  {}",
                    style("\u{2713}").green().bold(),
                    styled_artifact,
                    style(version).white(),
                    style(source).dim(),
                    style("up-to-date").green()
                );
            }
            VersionStatus::Outdated { latest } => {
                println!(
                    "  {} {}  {}  {}  {}",
                    style("\u{2192}").yellow().bold(),
                    styled_artifact,
                    style(version).white(),
                    style(source).dim(),
                    style(format!("\u{2192} {latest}")).yellow()
                );
            }
            VersionStatus::Skipped => {
                println!(
                    "  {} {}  {}  {}  {}",
                    style("-").dim().bold(),
                    styled_artifact,
                    style(version).dim(),
                    style(source).dim(),
                    style("skipped").dim()
                );
            }
            VersionStatus::Error { message } => {
                println!(
                    "  {} {}  {}  {}  {}",
                    style("\u{2717}").red().bold(),
                    styled_artifact,
                    style(version).white(),
                    style(source).dim(),
                    style(message).red()
                );
            }
        }
    }
}

impl OutputLine for UpdateResult {
    fn version_label(&self) -> String {
        let old = format_version_with_property(&self.old_version, self.property());
        format!("{old} \u{2192} {}", self.new_version)
    }

    fn print_line(&self) {
        let (styled_artifact, version, source) = format_columns(self);
        match &self.status {
            crate::dependency::UpdateStatus::Updated => {
                println!(
                    "  {} {}  {}  {}  {}",
                    style("\u{2713}").green().bold(),
                    styled_artifact,
                    style(version).white(),
                    style(source).dim(),
                    style("updated").green()
                );
            }
            crate::dependency::UpdateStatus::Error { message } => {
                println!(
                    "  {} {}  {}  {}  {}",
                    style("\u{2717}").red().bold(),
                    styled_artifact,
                    style(version).white(),
                    style(source).dim(),
                    style(message).red()
                );
            }
        }
    }
}

impl OutputLine for AuditResult {
    fn version_label(&self) -> String {
        format_version_with_property(&self.version, self.property())
    }

    fn print_line(&self) {
        let (styled_artifact, version, source) = format_columns(self);
        if self.vulnerabilities.is_empty() {
            println!(
                "  {} {}  {}  {}  {}",
                style("\u{2713}").green().bold(),
                styled_artifact,
                style(version).white(),
                style(source).dim(),
                style("no vulnerabilities").green()
            );
        } else {
            let count = self.vulnerabilities.len();
            let max_sev = self.max_severity();
            let label = if count == 1 {
                "vulnerability"
            } else {
                "vulnerabilities"
            };
            println!(
                "  {} {}  {}  {}  {}",
                style("\u{2717}").red().bold(),
                styled_artifact,
                style(version).white(),
                style(source).dim(),
                severity_style(max_sev).apply_to(format!("{count} {label}")),
            );
            for vuln in &self.vulnerabilities {
                let id_and_aliases = if vuln.aliases.is_empty() {
                    vuln.id.clone()
                } else {
                    format!("{} ({})", vuln.id, vuln.aliases.join(", "))
                };
                let summary = if vuln.summary.is_empty() {
                    String::new()
                } else {
                    let truncated = truncate_middle_pad(&vuln.summary, 60);
                    format!(" {truncated}")
                };
                println!(
                    "      {} {}{}",
                    severity_style(vuln.severity).apply_to(format!("[{}]", vuln.severity)),
                    style(id_and_aliases).dim(),
                    style(summary).dim(),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dependency::{Dependency, DependencyKind, Ecosystem};

    #[test]
    fn version_label_check_with_property() {
        let r = VersionResult::checked(
            Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                "g:a".into(),
                Some("version.junit".into()),
                String::new(),
            ),
            "5.10.0".into(),
            "5.12.0".into(),
            true,
        );
        assert_eq!(r.version_label(), "5.10.0 (version.junit)");
    }

    #[test]
    fn version_label_update_with_property() {
        let check = VersionResult::checked(
            Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                "g:a".into(),
                Some("version.junit".into()),
                String::new(),
            ),
            "5.10.0".into(),
            "5.12.0".into(),
            true,
        );
        let update = UpdateResult::updated(&check, "5.12.0".into());
        assert_eq!(
            update.version_label(),
            "5.10.0 (version.junit) \u{2192} 5.12.0"
        );
    }

    #[test]
    fn version_label_audit_with_property() {
        let r = AuditResult {
            id: Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                "g:a".into(),
                Some("version.junit".into()),
                String::new(),
            ),
            version: "5.10.0".into(),
            vulnerabilities: Vec::new(),
        };
        assert_eq!(r.version_label(), "5.10.0 (version.junit)");
    }
}
