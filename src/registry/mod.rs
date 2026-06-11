pub mod maven;

use anyhow::Result;
use async_trait::async_trait;

use crate::discovery::ArtifactMapping;

#[derive(Debug, Clone)]
pub struct CheckResult {
    pub property_name: String,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub outdated: bool,
    pub error: Option<String>,
    pub artifact: Option<String>,
}

#[async_trait]
pub trait VersionChecker: Send + Sync {
    async fn check(&self, mapping: &ArtifactMapping) -> Result<CheckResult>;
}
