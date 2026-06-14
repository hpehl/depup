use std::time::Duration;

pub const MAVEN_CENTRAL_URL: &str = "https://repo1.maven.org/maven2";
pub const NODEJS_DIST_URL: &str = "https://nodejs.org/dist/index.json";
pub const NPM_REGISTRY_URL: &str = "https://registry.npmjs.org";
pub const MAX_CONCURRENT_REQUESTS: usize = 10;
pub const HTTP_TIMEOUT_SECS: u64 = 30;

pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(format!("depup/{}", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()
        .expect("Failed to create HTTP client")
}
