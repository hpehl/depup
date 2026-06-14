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
    pub fn color(&self) -> console::Style {
        match self {
            Self::Dependency => console::Style::new().cyan(),
            Self::Plugin => console::Style::new().magenta(),
            Self::Node => console::Style::new().green(),
            Self::Npm => console::Style::new().yellow(),
            Self::Pnpm => console::Style::new().blue(),
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Dependency => "\u{25cf}",
            Self::Plugin => "\u{25a0}",
            Self::Node => "\u{25b2}",
            Self::Npm => "\u{25c6}",
            Self::Pnpm => "\u{25c6}",
        }
    }
}

use crate::progress::ProgressOutcome;

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
    pub kind: CheckerKind,
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
            kind,
        }
    }

    pub fn outcome(&self) -> ProgressOutcome<'_> {
        if let Some(err) = &self.error {
            ProgressOutcome::Error { message: err }
        } else if self.skipped {
            ProgressOutcome::Skipped
        } else if self.outdated {
            ProgressOutcome::Outdated {
                latest: self.latest_version.as_deref().unwrap_or("?"),
            }
        } else {
            ProgressOutcome::UpToDate
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
}
