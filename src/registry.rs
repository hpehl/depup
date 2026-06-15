#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Ecosystem {
    Maven,
    Npm,
}

impl std::fmt::Display for Ecosystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Maven => write!(f, "Maven"),
            Self::Npm => write!(f, "npm"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CheckerKind {
    Dependency,
    Plugin,
    Node,
    NpmPkg,
    NpmDep,
    NpmDevDep,
}

impl std::fmt::Display for CheckerKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dependency => write!(f, "Dependency"),
            Self::Plugin => write!(f, "Plugin"),
            Self::Node => write!(f, "Node"),
            Self::NpmPkg => write!(f, "npm"),
            Self::NpmDep => write!(f, "Dependency"),
            Self::NpmDevDep => write!(f, "Dev Dependency"),
        }
    }
}

impl CheckerKind {
    pub fn color(self) -> console::Style {
        match self {
            Self::Dependency => console::Style::new().cyan(),
            Self::Plugin => console::Style::new().magenta(),
            Self::Node => console::Style::new().green(),
            Self::NpmPkg => console::Style::new().yellow(),
            Self::NpmDep | Self::NpmDevDep => console::Style::new().blue(),
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            Self::Dependency => "\u{25cf}",
            Self::Plugin => "\u{25a0}",
            Self::Node => "\u{25b2}",
            Self::NpmPkg | Self::NpmDep | Self::NpmDevDep => "\u{25c6}",
        }
    }

    pub fn group_label(self) -> &'static str {
        match self {
            Self::Dependency => "Dependencies",
            Self::Plugin => "Plugins",
            Self::Node => "Node",
            Self::NpmPkg => "npm",
            Self::NpmDep => "Dependencies",
            Self::NpmDevDep => "Dev Dependencies",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CheckResult {
    pub ecosystem: Ecosystem,
    pub property_name: String,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub outdated: bool,
    pub skipped: bool,
    pub error: Option<String>,
    pub artifact: Option<String>,
    pub source: String,
    pub kind: CheckerKind,
}

pub enum Outcome<'a> {
    UpToDate,
    Outdated { latest: &'a str },
    Skipped,
    Error { message: &'a str },
}

impl CheckResult {
    #[allow(clippy::too_many_arguments)]
    pub fn checked(
        ecosystem: Ecosystem,
        kind: CheckerKind,
        property_name: String,
        current_version: String,
        latest_version: String,
        is_outdated: bool,
        artifact: Option<String>,
        source: String,
    ) -> Self {
        Self {
            ecosystem,
            property_name,
            current_version,
            latest_version: Some(latest_version),
            outdated: is_outdated,
            skipped: false,
            error: None,
            artifact,
            source,
            kind,
        }
    }

    pub fn skipped(
        ecosystem: Ecosystem,
        kind: CheckerKind,
        property_name: String,
        current_version: String,
        artifact: Option<String>,
        source: String,
    ) -> Self {
        Self {
            ecosystem,
            property_name,
            current_version,
            latest_version: None,
            outdated: false,
            skipped: true,
            error: None,
            artifact,
            source,
            kind,
        }
    }

    pub fn error(
        ecosystem: Ecosystem,
        kind: CheckerKind,
        property_name: String,
        current_version: String,
        artifact: Option<String>,
        error: String,
        source: String,
    ) -> Self {
        Self {
            ecosystem,
            property_name,
            current_version,
            latest_version: None,
            outdated: false,
            skipped: false,
            error: Some(error),
            artifact,
            source,
            kind,
        }
    }

    pub fn outcome(&self) -> Outcome<'_> {
        if let Some(err) = &self.error {
            Outcome::Error { message: err }
        } else if self.skipped {
            Outcome::Skipped
        } else if self.outdated {
            Outcome::Outdated {
                latest: self.latest_version.as_deref().unwrap_or("?"),
            }
        } else {
            Outcome::UpToDate
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecosystem_display() {
        assert_eq!(Ecosystem::Maven.to_string(), "Maven");
        assert_eq!(Ecosystem::Npm.to_string(), "npm");
    }

    #[test]
    fn checked_result_fields() {
        let r = CheckResult::checked(
            Ecosystem::Maven,
            CheckerKind::Dependency,
            "version.junit".to_string(),
            "5.10.0".to_string(),
            "5.12.0".to_string(),
            true,
            Some("org.junit:junit".to_string()),
            String::new(),
        );
        assert!(r.outdated);
        assert!(!r.skipped);
        assert!(r.error.is_none());
        assert_eq!(r.latest_version.as_deref(), Some("5.12.0"));
    }

    #[test]
    fn skipped_result_fields() {
        let r = CheckResult::skipped(
            Ecosystem::Maven,
            CheckerKind::Dependency,
            "version.alpha".to_string(),
            "1.0.0-alpha".to_string(),
            None,
            String::new(),
        );
        assert!(r.skipped);
        assert!(!r.outdated);
        assert!(r.latest_version.is_none());
    }

    #[test]
    fn error_result_fields() {
        let r = CheckResult::error(
            Ecosystem::Maven,
            CheckerKind::Plugin,
            "version.plugin".to_string(),
            "1.0".to_string(),
            None,
            "timeout".to_string(),
            String::new(),
        );
        assert!(r.error.is_some());
        assert!(!r.outdated);
        assert!(!r.skipped);
    }

    #[test]
    fn outcome_matches_state() {
        let up_to_date = CheckResult::checked(
            Ecosystem::Maven,
            CheckerKind::Dependency,
            "p".into(),
            "1.0".into(),
            "1.0".into(),
            false,
            None,
            String::new(),
        );
        assert!(matches!(up_to_date.outcome(), Outcome::UpToDate));

        let outdated = CheckResult::checked(
            Ecosystem::Maven,
            CheckerKind::Dependency,
            "p".into(),
            "1.0".into(),
            "2.0".into(),
            true,
            None,
            String::new(),
        );
        assert!(matches!(outdated.outcome(), Outcome::Outdated { .. }));

        let skipped = CheckResult::skipped(
            Ecosystem::Maven,
            CheckerKind::Dependency,
            "p".into(),
            "1.0".into(),
            None,
            String::new(),
        );
        assert!(matches!(skipped.outcome(), Outcome::Skipped));

        let error = CheckResult::error(
            Ecosystem::Maven,
            CheckerKind::Dependency,
            "p".into(),
            "1.0".into(),
            None,
            "fail".into(),
            String::new(),
        );
        assert!(matches!(error.outcome(), Outcome::Error { .. }));
    }

    #[test]
    fn checker_kind_display() {
        assert_eq!(CheckerKind::Dependency.to_string(), "Dependency");
        assert_eq!(CheckerKind::Plugin.to_string(), "Plugin");
        assert_eq!(CheckerKind::Node.to_string(), "Node");
        assert_eq!(CheckerKind::NpmPkg.to_string(), "npm");
        assert_eq!(CheckerKind::NpmDep.to_string(), "Dependency");
        assert_eq!(CheckerKind::NpmDevDep.to_string(), "Dev Dependency");
    }
}
