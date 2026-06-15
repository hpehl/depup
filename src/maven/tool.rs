//! Tool version checker trait and registry.
//!
//! Provides an extensible mechanism for checking non-Maven version properties
//! found in POM files (e.g., `version.node`, `version.pnpm`). Each checker
//! declares the property name patterns it handles and performs its own
//! registry lookup.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;

use crate::dependency::VersionResult;
use crate::maven::discovery::VersionProperty;
use crate::maven::node::NodeChecker;
use crate::maven::pm_versions::PmVersionsChecker;

/// Trait for checking tool version properties against their respective registries.
pub trait ToolVersionChecker: Send + Sync {
    /// Returns the property name patterns this checker handles.
    fn patterns(&self) -> &[&str];
    /// Returns a display label for progress reporting.
    fn label(&self, property: &VersionProperty) -> String;
    /// Checks the property's current value against the latest available version.
    fn check<'a>(
        &'a self,
        property: &'a VersionProperty,
        source: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<VersionResult>> + Send + 'a>>;
}

/// Registry of all available tool version checkers.
/// Matches orphan POM properties to the appropriate checker by pattern.
pub struct ToolCheckerRegistry {
    checkers: Vec<Arc<dyn ToolVersionChecker>>,
}

impl ToolCheckerRegistry {
    pub fn new(stable: bool) -> Self {
        Self {
            checkers: vec![
                Arc::new(NodeChecker::new(stable)),
                Arc::new(PmVersionsChecker::new()),
            ],
        }
    }

    /// Finds the first checker whose patterns include the given property name.
    pub fn find(&self, property_name: &str) -> Option<Arc<dyn ToolVersionChecker>> {
        self.checkers
            .iter()
            .find(|c| c.patterns().contains(&property_name))
            .cloned()
    }
}
