use super::{Dependency, CommandResult, DependencyKind, Ecosystem};

/// The outcome of checking a dependency version.
#[derive(Debug, Clone)]
pub enum CheckStatus {
    UpToDate { latest: String },
    Outdated { latest: String },
    Skipped,
    Error { message: String },
}

// ------------------------------------------------------ command result

/// Result of checking a single dependency against its registry.
#[derive(Debug, Clone)]
pub struct CheckResult {
    pub dep: Dependency,
    pub current_version: String,
    pub status: CheckStatus,
}

impl CommandResult for CheckResult {
    fn ecosystem(&self) -> Ecosystem {
        self.dep.ecosystem
    }
    fn kind(&self) -> DependencyKind {
        self.dep.kind
    }
    fn artifact(&self) -> &str {
        &self.dep.artifact
    }
    fn property(&self) -> Option<&str> {
        self.dep.property.as_deref()
    }
    fn source(&self) -> &str {
        &self.dep.source
    }
}

impl CheckResult {
    pub fn checked(id: Dependency, current: String, latest: String, is_outdated: bool) -> Self {
        let status = if is_outdated {
            CheckStatus::Outdated { latest }
        } else {
            CheckStatus::UpToDate { latest }
        };
        Self {
            dep: id,
            current_version: current,
            status,
        }
    }

    pub fn skipped(id: Dependency, current: String) -> Self {
        Self {
            dep: id,
            current_version: current,
            status: CheckStatus::Skipped,
        }
    }

    pub fn error(id: Dependency, current: String, message: String) -> Self {
        Self {
            dep: id,
            current_version: current,
            status: CheckStatus::Error { message },
        }
    }

    pub fn is_outdated(&self) -> bool {
        matches!(self.status, CheckStatus::Outdated { .. })
    }

    pub fn is_skipped(&self) -> bool {
        matches!(self.status, CheckStatus::Skipped)
    }

    pub fn error_message(&self) -> Option<&str> {
        match &self.status {
            CheckStatus::Error { message } => Some(message),
            _ => None,
        }
    }

    pub fn latest_version(&self) -> Option<&str> {
        match &self.status {
            CheckStatus::UpToDate { latest } | CheckStatus::Outdated { latest } => Some(latest),
            _ => None,
        }
    }
}

// ------------------------------------------------------ test

#[cfg(test)]
mod tests {
    use super::*;

    fn dep_id() -> Dependency {
        Dependency::new(
            Ecosystem::Maven,
            DependencyKind::Dependency,
            "org.junit:junit".to_string(),
            Some("version.junit".to_string()),
            String::new(),
        )
    }

    #[test]
    fn checked_result_fields() {
        let r = CheckResult::checked(dep_id(), "5.10.0".to_string(), "5.12.0".to_string(), true);
        assert!(r.is_outdated());
        assert!(!r.is_skipped());
        assert!(r.error_message().is_none());
        assert_eq!(r.latest_version(), Some("5.12.0"));
    }

    #[test]
    fn skipped_result_fields() {
        let id = Dependency::new(
            Ecosystem::Maven,
            DependencyKind::Dependency,
            "org.alpha:alpha".to_string(),
            Some("version.alpha".to_string()),
            String::new(),
        );
        let r = CheckResult::skipped(id, "1.0.0-alpha".to_string());
        assert!(r.is_skipped());
        assert!(!r.is_outdated());
        assert!(r.latest_version().is_none());
    }

    #[test]
    fn error_result_fields() {
        let id = Dependency::new(
            Ecosystem::Maven,
            DependencyKind::Plugin,
            "org.plugin:plugin".to_string(),
            Some("version.plugin".to_string()),
            String::new(),
        );
        let r = CheckResult::error(id, "1.0".to_string(), "timeout".to_string());
        assert!(r.error_message().is_some());
        assert!(!r.is_outdated());
        assert!(!r.is_skipped());
    }

    #[test]
    fn status_matches_state() {
        let up_to_date = CheckResult::checked(
            Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                "g:a".into(),
                None,
                String::new(),
            ),
            "1.0".into(),
            "1.0".into(),
            false,
        );
        assert!(matches!(up_to_date.status, CheckStatus::UpToDate { .. }));

        let outdated = CheckResult::checked(
            Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                "g:a".into(),
                None,
                String::new(),
            ),
            "1.0".into(),
            "2.0".into(),
            true,
        );
        assert!(matches!(outdated.status, CheckStatus::Outdated { .. }));

        let skipped = CheckResult::skipped(
            Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                "g:a".into(),
                None,
                String::new(),
            ),
            "1.0".into(),
        );
        assert!(matches!(skipped.status, CheckStatus::Skipped));

        let error = CheckResult::error(
            Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                "g:a".into(),
                None,
                String::new(),
            ),
            "1.0".into(),
            "fail".into(),
        );
        assert!(matches!(error.status, CheckStatus::Error { .. }));
    }
}
