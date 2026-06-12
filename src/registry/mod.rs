pub mod maven;

#[derive(Debug, Clone)]
pub struct CheckResult {
    pub property_name: String,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub outdated: bool,
    pub error: Option<String>,
    pub artifact: Option<String>,
}
