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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CheckerKind {
    Dependency,
    Plugin,
    Node,
    Npm,
    Pnpm,
}

impl std::fmt::Display for CheckerKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dependency => write!(f, "Dependency"),
            Self::Plugin => write!(f, "Plugin"),
            Self::Node => write!(f, "Node"),
            Self::Npm => write!(f, "npm"),
            Self::Pnpm => write!(f, "pnpm"),
        }
    }
}

impl CheckerKind {
    pub fn color(self) -> console::Style {
        match self {
            Self::Dependency => console::Style::new().cyan(),
            Self::Plugin => console::Style::new().magenta(),
            Self::Node => console::Style::new().green(),
            Self::Npm => console::Style::new().yellow(),
            Self::Pnpm => console::Style::new().blue(),
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            Self::Dependency => "\u{25cf}",
            Self::Plugin => "\u{25a0}",
            Self::Node => "\u{25b2}",
            Self::Npm | Self::Pnpm => "\u{25c6}",
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
    pub fn checked(
        ecosystem: Ecosystem,
        kind: CheckerKind,
        property_name: String,
        current_version: String,
        latest_version: String,
        is_outdated: bool,
        artifact: Option<String>,
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
            source: String::new(),
            kind,
        }
    }

    pub fn skipped(
        ecosystem: Ecosystem,
        kind: CheckerKind,
        property_name: String,
        current_version: String,
        artifact: Option<String>,
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
            source: String::new(),
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
            source: String::new(),
            kind,
        }
    }

    pub fn with_source(mut self, source: String) -> Self {
        self.source = source;
        self
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
        assert_eq!(Ecosystem::Pnpm.to_string(), "pnpm");
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
        );
        assert!(matches!(outdated.outcome(), Outcome::Outdated { .. }));

        let skipped = CheckResult::skipped(
            Ecosystem::Maven,
            CheckerKind::Dependency,
            "p".into(),
            "1.0".into(),
            None,
        );
        assert!(matches!(skipped.outcome(), Outcome::Skipped));

        let error = CheckResult::error(
            Ecosystem::Maven,
            CheckerKind::Dependency,
            "p".into(),
            "1.0".into(),
            None,
            "fail".into(),
        );
        assert!(matches!(error.outcome(), Outcome::Error { .. }));
    }

    #[test]
    fn checker_kind_display() {
        assert_eq!(CheckerKind::Dependency.to_string(), "Dependency");
        assert_eq!(CheckerKind::Plugin.to_string(), "Plugin");
        assert_eq!(CheckerKind::Node.to_string(), "Node");
        assert_eq!(CheckerKind::Npm.to_string(), "npm");
        assert_eq!(CheckerKind::Pnpm.to_string(), "pnpm");
    }
}
