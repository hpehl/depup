//! Tool version resolver trait and registry.
//!
//! Provides an extensible mechanism for resolving non-Maven version properties
//! found in POM files (e.g., `version.node`, `version.pnpm`). Each resolver
//! declares the property name patterns it handles and performs its own
//! registry lookup.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;

use crate::dependency::VersionResult;
use crate::maven::discovery::VersionProperty;
use crate::maven::node::NodeResolver;
use crate::maven::pm_versions::PmVersionsResolver;

/// Trait for resolving tool version properties against their respective registries.
pub trait ToolVersionResolver: Send + Sync {
    /// Returns the property name patterns this resolver handles.
    fn patterns(&self) -> &[&str];
    /// Returns a display label for progress reporting.
    fn label(&self, property: &VersionProperty) -> String;
    /// Resolves the property's current value against the latest available version.
    fn resolve<'a>(
        &'a self,
        property: &'a VersionProperty,
        source: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<VersionResult>> + Send + 'a>>;
}

/// Registry of all available tool version resolvers.
/// Matches orphan POM properties to the appropriate resolver by pattern.
pub struct ToolResolverRegistry {
    resolvers: Vec<Arc<dyn ToolVersionResolver>>,
}

impl ToolResolverRegistry {
    pub fn new(stable: bool) -> Self {
        Self {
            resolvers: vec![
                Arc::new(NodeResolver::new(stable)),
                Arc::new(PmVersionsResolver::new()),
            ],
        }
    }

    /// Finds the first resolver whose patterns include the given property name.
    pub fn find(&self, property_name: &str) -> Option<Arc<dyn ToolVersionResolver>> {
        self.resolvers
            .iter()
            .find(|c| c.patterns().contains(&property_name))
            .cloned()
    }
}
