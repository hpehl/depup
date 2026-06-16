use super::check::VersionResult;
use super::{Dependency, DependencyInfo, DependencyKind, Ecosystem};

/// The outcome of updating a dependency version.
#[derive(Debug, Clone)]
pub enum UpdateStatus {
    Updated,
    Error { message: String },
}

/// Result of updating a single dependency.
#[derive(Debug, Clone)]
pub struct UpdateResult {
    pub dep: Dependency,
    pub old_version: String,
    pub new_version: String,
    pub status: UpdateStatus,
}

impl DependencyInfo for UpdateResult {
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

impl UpdateResult {
    pub fn updated(check: &VersionResult, new_version: String) -> Self {
        Self {
            dep: check.dep.clone(),
            old_version: check.current_version.clone(),
            new_version,
            status: UpdateStatus::Updated,
        }
    }

    pub fn error(check: &VersionResult, message: String) -> Self {
        let new_version = check.latest_version().unwrap_or("").to_string();
        Self {
            dep: check.dep.clone(),
            old_version: check.current_version.clone(),
            new_version,
            status: UpdateStatus::Error { message },
        }
    }

    pub fn is_error(&self) -> bool {
        matches!(self.status, UpdateStatus::Error { .. })
    }

    #[cfg(test)]
    pub fn error_message(&self) -> Option<&str> {
        match &self.status {
            UpdateStatus::Error { message } => Some(message),
            _ => None,
        }
    }
}

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
    fn update_result_from_check_result() {
        let check = VersionResult::checked(dep_id(), "5.10.0".into(), "5.12.0".into(), true);
        let update = UpdateResult::updated(&check, "5.12.0".into());
        assert_eq!(update.dep.ecosystem, Ecosystem::Maven);
        assert_eq!(update.old_version, "5.10.0");
        assert_eq!(update.new_version, "5.12.0");
        assert!(!update.is_error());
    }

    #[test]
    fn update_result_error() {
        let check = VersionResult::checked(dep_id(), "5.10.0".into(), "5.12.0".into(), true);
        let update = UpdateResult::error(&check, "write failed".into());
        assert!(update.is_error());
        assert_eq!(update.error_message(), Some("write failed"));
    }
}
