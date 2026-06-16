//! Global constants and shared HTTP client factory.

use std::sync::LazyLock;
use std::time::Duration;

/// Maven Central base URL for fetching `maven-metadata.xml`.
pub const MAVEN_CENTRAL_URL: &str = "https://repo1.maven.org/maven2";

/// Node.js distribution index URL listing all releases.
pub const NODEJS_DIST_URL: &str = "https://nodejs.org/dist/index.json";

/// npm registry base URL for package metadata lookups.
pub const NPM_REGISTRY_URL: &str = "https://registry.npmjs.org";

/// Semaphore limit for concurrent HTTP requests and subprocess spawns.
pub const MAX_CONCURRENT_REQUESTS: usize = 10;

/// HTTP request timeout in seconds.
pub const HTTP_TIMEOUT_SECS: u64 = 30;

static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .user_agent(format!("depup/{}", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()
        .expect("Failed to create HTTP client")
});

/// Returns a shared HTTP client with a `depup/{version}` user agent.
/// Maven Central returns 403 without a proper User-Agent header.
pub fn http_client() -> reqwest::Client {
    HTTP_CLIENT.clone()
}
