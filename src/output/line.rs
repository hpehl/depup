use console::style;

use crate::model::{AuditResult, CheckResult, CheckStatus, CommandResult, UpdateResult};

use super::format::{format_columns, severity_style, truncate_middle_pad};

/// Subcommand-specific line rendering. Each result type provides its own
/// version label and formatted output while sharing the common column layout.
pub(crate) trait OutputLine: CommandResult {
    fn version_value(&self) -> String;
    fn format_line(&self) -> String;
}

impl OutputLine for CheckResult {
    fn version_value(&self) -> String {
        self.current_version.clone()
    }

    fn format_line(&self) -> String {
        let (styled_artifact, version, source) = format_columns(self);
        match &self.status {
            CheckStatus::UpToDate { .. } => {
                format!(
                    "  {} {}  {}  {}  {}",
                    style("\u{2713}").green().bold(),
                    styled_artifact,
                    style(version).white(),
                    style(source).dim(),
                    style("up-to-date").green()
                )
            }
            CheckStatus::Outdated { latest } => {
                format!(
                    "  {} {}  {}  {}  {}",
                    style("\u{2192}").yellow().bold(),
                    styled_artifact,
                    style(version).white(),
                    style(source).dim(),
                    style(format!("\u{2192} {latest}")).yellow()
                )
            }
            CheckStatus::Skipped => {
                format!(
                    "  {} {}  {}  {}  {}",
                    style("-").dim().bold(),
                    styled_artifact,
                    style(version).dim(),
                    style(source).dim(),
                    style("skipped").dim()
                )
            }
            CheckStatus::Error { message } => {
                format!(
                    "  {} {}  {}  {}  {}",
                    style("\u{2717}").red().bold(),
                    styled_artifact,
                    style(version).white(),
                    style(source).dim(),
                    style(message).red()
                )
            }
        }
    }
}

impl OutputLine for UpdateResult {
    fn version_value(&self) -> String {
        format!("{} \u{2192} {}", self.old_version, self.new_version)
    }

    fn format_line(&self) -> String {
        let (styled_artifact, version, source) = format_columns(self);
        match &self.status {
            crate::model::UpdateStatus::Updated => {
                format!(
                    "  {} {}  {}  {}  {}",
                    style("\u{2713}").green().bold(),
                    styled_artifact,
                    style(version).white(),
                    style(source).dim(),
                    style("updated").green()
                )
            }
            crate::model::UpdateStatus::Error { message } => {
                format!(
                    "  {} {}  {}  {}  {}",
                    style("\u{2717}").red().bold(),
                    styled_artifact,
                    style(version).white(),
                    style(source).dim(),
                    style(message).red()
                )
            }
        }
    }
}

impl OutputLine for AuditResult {
    fn version_value(&self) -> String {
        self.version.clone()
    }

    fn format_line(&self) -> String {
        let (styled_artifact, version, source) = format_columns(self);
        if self.vulnerabilities.is_empty() {
            return format!(
                "  {} {}  {}  {}  {}",
                style("\u{2713}").green().bold(),
                styled_artifact,
                style(version).white(),
                style(source).dim(),
                style("no vulnerabilities").green()
            );
        }

        let count = self.vulnerabilities.len();
        let max_sev = self.max_severity();
        let label = if count == 1 {
            "vulnerability"
        } else {
            "vulnerabilities"
        };

        let mut lines = vec![format!(
            "  {} {}  {}  {}  {}",
            style("\u{2717}").red().bold(),
            styled_artifact,
            style(version).white(),
            style(source).dim(),
            severity_style(max_sev).apply_to(format!("{count} {label}")),
        )];

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
            lines.push(format!(
                "      {} {}{}",
                severity_style(vuln.severity).apply_to(format!("[{}]", vuln.severity)),
                style(id_and_aliases).dim(),
                style(summary).dim(),
            ));
        }

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Dependency, DependencyKind, Ecosystem};

    #[test]
    fn version_value_check() {
        let r = CheckResult::checked(
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
        assert_eq!(r.version_value(), "5.10.0");
    }

    #[test]
    fn version_value_update() {
        let check = CheckResult::checked(
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
        assert_eq!(update.version_value(), "5.10.0 \u{2192} 5.12.0");
    }

    #[test]
    fn version_value_audit() {
        let r = AuditResult {
            dep: Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                "g:a".into(),
                Some("version.junit".into()),
                String::new(),
            ),
            version: "5.10.0".into(),
            vulnerabilities: Vec::new(),
        };
        assert_eq!(r.version_value(), "5.10.0");
    }

    #[test]
    fn format_line_check_contains_artifact() {
        let r = CheckResult::checked(
            Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                "org.junit:junit".into(),
                None,
                "pom.xml".into(),
            ),
            "5.10.0".into(),
            "5.12.0".into(),
            true,
        );
        let line = r.format_line();
        assert!(line.contains("org.junit:junit"));
        assert!(line.contains("5.10.0"));
    }

    #[test]
    fn format_line_update_contains_versions() {
        let check = CheckResult::checked(
            Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                "g:a".into(),
                None,
                String::new(),
            ),
            "1.0.0".into(),
            "2.0.0".into(),
            true,
        );
        let update = UpdateResult::updated(&check, "2.0.0".into());
        let line = update.format_line();
        assert!(line.contains("1.0.0"));
        assert!(line.contains("2.0.0"));
    }

    #[test]
    fn format_line_audit_no_vulns() {
        let r = AuditResult {
            dep: Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                "g:a".into(),
                None,
                String::new(),
            ),
            version: "1.0.0".into(),
            vulnerabilities: Vec::new(),
        };
        let line = r.format_line();
        assert!(line.contains("no vulnerabilities"));
    }
}
