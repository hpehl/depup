//! OSV.dev API client for querying known vulnerabilities.
//!
//! Uses the batch query endpoint to check multiple packages at once,
//! then fetches full vulnerability details for each unique ID.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::Result;
use indicatif::ProgressBar;
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::constants::{MAX_CONCURRENT_REQUESTS, http_client};
use crate::model::{
    AuditResult, CommandResult, DependencyKind, Ecosystem, Severity, VersionResult, Vulnerability,
};

const OSV_API_URL: &str = "https://api.osv.dev";
/// OSV.dev recommends batching queries. The API accepts up to 1000 per request;
/// 500 balances request size against response latency.
const BATCH_CHUNK_SIZE: usize = 500;

// ---------------------------------------------------------------------------
// OSV API request/response types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct OsvQuery {
    package: OsvPackage,
    version: String,
}

#[derive(Serialize)]
struct OsvPackage {
    name: String,
    ecosystem: String,
}

#[derive(Deserialize)]
struct OsvBatchResponse {
    results: Vec<OsvQueryResult>,
}

#[derive(Deserialize)]
struct OsvQueryResult {
    #[serde(default)]
    vulns: Vec<OsvVulnRef>,
}

#[derive(Deserialize)]
struct OsvVulnRef {
    id: String,
}

#[derive(Deserialize)]
struct OsvVulnerability {
    id: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    severity: Vec<OsvSeverity>,
    #[serde(default)]
    references: Vec<OsvReference>,
    #[serde(default)]
    affected: Vec<OsvAffected>,
}

#[derive(Deserialize)]
struct OsvSeverity {
    #[serde(default)]
    score: Option<String>,
}

#[derive(Deserialize)]
struct OsvReference {
    #[serde(rename = "type", default)]
    ref_type: String,
    #[serde(default)]
    url: String,
}

#[derive(Deserialize)]
struct OsvAffected {
    #[serde(default)]
    ecosystem_specific: Option<OsvEcosystemSpecific>,
    #[serde(default)]
    database_specific: Option<OsvDatabaseSpecific>,
}

#[derive(Deserialize)]
struct OsvEcosystemSpecific {
    #[serde(default)]
    severity: Option<String>,
}

#[derive(Deserialize)]
struct OsvDatabaseSpecific {
    #[serde(default)]
    severity: Option<String>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Audits dependencies against OSV.dev and returns results with full vulnerability details.
pub async fn audit(results: &[VersionResult], bar: &ProgressBar) -> Result<Vec<AuditResult>> {
    let auditable: Vec<&VersionResult> = results
        .iter()
        .filter(|r| {
            r.kind() != DependencyKind::Tool
                && !r.is_skipped()
                && r.error_message().is_none()
                && !r.current_version.is_empty()
        })
        .collect();

    if auditable.is_empty() {
        return Ok(Vec::new());
    }

    bar.set_message("Querying OSV.dev...");

    // Phase 1: Batch query for vulnerability IDs
    let vuln_map = query_batch(&auditable).await?;
    bar.inc(1);

    // Collect unique vuln IDs across all results
    let unique_ids: HashSet<String> = vuln_map.values().flatten().cloned().collect();

    if unique_ids.is_empty() {
        bar.inc(1);
        return Ok(auditable
            .iter()
            .map(|r| AuditResult::from_version_result(r, Vec::new()))
            .collect());
    }

    // Phase 2: Fetch full vulnerability details
    bar.set_message("Fetching vulnerability details...");
    let vuln_details = fetch_vulnerabilities(unique_ids).await;
    bar.inc(1);

    // Phase 3: Join deps with their vulnerabilities
    let audit_results = auditable
        .iter()
        .map(|r| {
            let key = dep_key(r);
            let vulns = vuln_map
                .get(&key)
                .map(|ids| {
                    ids.iter()
                        .filter_map(|id| vuln_details.get(id))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            AuditResult::from_version_result(r, vulns)
        })
        .collect();

    Ok(audit_results)
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

fn dep_key(r: &VersionResult) -> String {
    format!("{}:{}:{}", r.ecosystem(), r.artifact(), r.current_version)
}

fn osv_ecosystem(ecosystem: Ecosystem) -> &'static str {
    match ecosystem {
        Ecosystem::Maven => "Maven",
        Ecosystem::Npm => "npm",
    }
}

async fn query_batch(deps: &[&VersionResult]) -> Result<HashMap<String, Vec<String>>> {
    let client = http_client();
    let mut result_map: HashMap<String, Vec<String>> = HashMap::new();

    // Build query list, deduplicating by (ecosystem, artifact, version)
    let mut seen = HashSet::new();
    let mut queries: Vec<(String, OsvQuery)> = Vec::new();
    for dep in deps {
        let key = dep_key(dep);
        if seen.insert(key.clone()) {
            queries.push((
                key,
                OsvQuery {
                    package: OsvPackage {
                        name: dep.artifact().to_string(),
                        ecosystem: osv_ecosystem(dep.ecosystem()).to_string(),
                    },
                    version: dep.current_version.clone(),
                },
            ));
        }
    }

    for chunk in queries.chunks(BATCH_CHUNK_SIZE) {
        let batch_queries: Vec<&OsvQuery> = chunk.iter().map(|(_, q)| q).collect();
        let request = serde_json::json!({
            "queries": batch_queries,
        });

        let response = client
            .post(format!("{OSV_API_URL}/v1/querybatch"))
            .json(&request)
            .send()
            .await?
            .error_for_status()?
            .json::<OsvBatchResponse>()
            .await?;

        for (i, query_result) in response.results.iter().enumerate() {
            if let Some((key, _)) = chunk.get(i) {
                let ids: Vec<String> = query_result.vulns.iter().map(|v| v.id.clone()).collect();
                if !ids.is_empty() {
                    result_map.insert(key.clone(), ids);
                }
            }
        }
    }

    // Map back to all deps (including duplicates)
    let mut full_map: HashMap<String, Vec<String>> = HashMap::new();
    for dep in deps {
        let key = dep_key(dep);
        if let Some(ids) = result_map.get(&key) {
            full_map.insert(key, ids.clone());
        }
    }

    Ok(full_map)
}

async fn fetch_vulnerabilities(ids: HashSet<String>) -> HashMap<String, Vulnerability> {
    let client = http_client();
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS));
    let mut join_set = JoinSet::new();

    for id in ids {
        let client = client.clone();
        let semaphore = Arc::clone(&semaphore);
        join_set.spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            let result = client
                .get(format!("{OSV_API_URL}/v1/vulns/{id}"))
                .send()
                .await
                .and_then(|r| r.error_for_status())
                .ok();

            if let Some(response) = result {
                response
                    .json::<OsvVulnerability>()
                    .await
                    .ok()
                    .map(|osv| (id, convert_vulnerability(osv)))
            } else {
                None
            }
        });
    }

    join_set.join_all().await.into_iter().flatten().collect()
}

fn convert_vulnerability(osv: OsvVulnerability) -> Vulnerability {
    let severity = extract_severity(&osv);
    let url = osv
        .references
        .iter()
        .find(|r| r.ref_type == "ADVISORY" || r.ref_type == "WEB")
        .map(|r| r.url.clone());

    Vulnerability {
        id: osv.id,
        aliases: osv.aliases,
        summary: osv.summary,
        severity,
        url,
    }
}

fn extract_severity(osv: &OsvVulnerability) -> Severity {
    // Try CVSS score first
    for sev in &osv.severity {
        if let Some(score_str) = &sev.score {
            // CVSS vector strings end with a score, or the score field itself is numeric
            if let Ok(score) = score_str.parse::<f64>() {
                return Severity::from_cvss(score);
            }
            // Try extracting score from CVSS vector (last component after /)
            if let Some(last) = score_str.rsplit('/').next() {
                if let Ok(score) = last.parse::<f64>() {
                    return Severity::from_cvss(score);
                }
            }
        }
    }

    // Try ecosystem_specific or database_specific severity labels
    for affected in &osv.affected {
        if let Some(es) = &affected.ecosystem_specific {
            if let Some(label) = &es.severity {
                return Severity::from_str_label(label);
            }
        }
        if let Some(ds) = &affected.database_specific {
            if let Some(label) = &ds.severity {
                return Severity::from_str_label(label);
            }
        }
    }

    Severity::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_from_cvss_thresholds() {
        assert_eq!(Severity::from_cvss(9.0), Severity::Critical);
        assert_eq!(Severity::from_cvss(10.0), Severity::Critical);
        assert_eq!(Severity::from_cvss(7.0), Severity::High);
        assert_eq!(Severity::from_cvss(8.9), Severity::High);
        assert_eq!(Severity::from_cvss(4.0), Severity::Medium);
        assert_eq!(Severity::from_cvss(6.9), Severity::Medium);
        assert_eq!(Severity::from_cvss(0.1), Severity::Low);
        assert_eq!(Severity::from_cvss(3.9), Severity::Low);
        assert_eq!(Severity::from_cvss(0.0), Severity::Unknown);
    }

    #[test]
    fn severity_from_string_label() {
        assert_eq!(Severity::from_str_label("CRITICAL"), Severity::Critical);
        assert_eq!(Severity::from_str_label("high"), Severity::High);
        assert_eq!(Severity::from_str_label("Medium"), Severity::Medium);
        assert_eq!(Severity::from_str_label("MODERATE"), Severity::Medium);
        assert_eq!(Severity::from_str_label("low"), Severity::Low);
        assert_eq!(Severity::from_str_label("something"), Severity::Unknown);
    }

    #[test]
    fn severity_ordering() {
        assert!(Severity::Critical > Severity::High);
        assert!(Severity::High > Severity::Medium);
        assert!(Severity::Medium > Severity::Low);
        assert!(Severity::Low > Severity::Unknown);
    }

    #[test]
    fn severity_display() {
        assert_eq!(Severity::Critical.to_string(), "CRITICAL");
        assert_eq!(Severity::High.to_string(), "HIGH");
        assert_eq!(Severity::Medium.to_string(), "MEDIUM");
        assert_eq!(Severity::Low.to_string(), "LOW");
        assert_eq!(Severity::Unknown.to_string(), "UNKNOWN");
    }

    #[test]
    fn osv_ecosystem_mapping() {
        assert_eq!(osv_ecosystem(Ecosystem::Maven), "Maven");
        assert_eq!(osv_ecosystem(Ecosystem::Npm), "npm");
    }

    #[test]
    fn convert_vulnerability_with_cvss() {
        let osv = OsvVulnerability {
            id: "GHSA-test-1234".into(),
            summary: "Test vulnerability".into(),
            aliases: vec!["CVE-2024-1234".into()],
            severity: vec![OsvSeverity {
                score: Some("9.8".into()),
            }],
            references: vec![OsvReference {
                ref_type: "ADVISORY".into(),
                url: "https://example.com/advisory".into(),
            }],
            affected: Vec::new(),
        };

        let vuln = convert_vulnerability(osv);
        assert_eq!(vuln.id, "GHSA-test-1234");
        assert_eq!(vuln.severity, Severity::Critical);
        assert_eq!(vuln.aliases, vec!["CVE-2024-1234"]);
        assert_eq!(vuln.url, Some("https://example.com/advisory".into()));
    }

    #[test]
    fn convert_vulnerability_with_ecosystem_severity() {
        let osv = OsvVulnerability {
            id: "OSV-2024-001".into(),
            summary: "Another vuln".into(),
            aliases: Vec::new(),
            severity: Vec::new(),
            references: Vec::new(),
            affected: vec![OsvAffected {
                ecosystem_specific: Some(OsvEcosystemSpecific {
                    severity: Some("HIGH".into()),
                }),
                database_specific: None,
            }],
        };

        let vuln = convert_vulnerability(osv);
        assert_eq!(vuln.severity, Severity::High);
        assert!(vuln.url.is_none());
    }

    #[test]
    fn convert_vulnerability_unknown_severity() {
        let osv = OsvVulnerability {
            id: "OSV-2024-002".into(),
            summary: String::new(),
            aliases: Vec::new(),
            severity: Vec::new(),
            references: Vec::new(),
            affected: Vec::new(),
        };

        let vuln = convert_vulnerability(osv);
        assert_eq!(vuln.severity, Severity::Unknown);
    }

    #[test]
    fn dep_key_format() {
        use crate::model::{Dependency, DependencyKind};
        let r = VersionResult::checked(
            Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                "org.junit:junit".into(),
                None,
                String::new(),
            ),
            "5.10.0".into(),
            "5.12.0".into(),
            true,
        );
        assert_eq!(dep_key(&r), "Maven:org.junit:junit:5.10.0");
    }

    #[test]
    fn dep_key_npm_format() {
        use crate::model::{Dependency, DependencyKind};
        let r = VersionResult::checked(
            Dependency::new(
                Ecosystem::Npm,
                DependencyKind::NpmDep,
                "lodash".into(),
                None,
                String::new(),
            ),
            "1.0.0".into(),
            "2.0.0".into(),
            true,
        );
        assert_eq!(dep_key(&r), "npm:lodash:1.0.0");
    }

    #[test]
    fn extract_severity_from_cvss_vector_string() {
        let osv = OsvVulnerability {
            id: "GHSA-vec-test".into(),
            summary: String::new(),
            aliases: Vec::new(),
            severity: vec![OsvSeverity {
                score: Some("CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H/9.8".into()),
            }],
            references: Vec::new(),
            affected: Vec::new(),
        };
        assert_eq!(extract_severity(&osv), Severity::Critical);
    }

    #[test]
    fn convert_vulnerability_with_database_specific_severity() {
        let osv = OsvVulnerability {
            id: "OSV-DB-001".into(),
            summary: "DB-specific vuln".into(),
            aliases: Vec::new(),
            severity: Vec::new(),
            references: Vec::new(),
            affected: vec![OsvAffected {
                ecosystem_specific: None,
                database_specific: Some(OsvDatabaseSpecific {
                    severity: Some("MEDIUM".into()),
                }),
            }],
        };

        let vuln = convert_vulnerability(osv);
        assert_eq!(vuln.severity, Severity::Medium);
    }

    #[test]
    fn convert_vulnerability_with_web_reference() {
        let osv = OsvVulnerability {
            id: "OSV-WEB-001".into(),
            summary: "Web ref vuln".into(),
            aliases: Vec::new(),
            severity: vec![OsvSeverity {
                score: Some("7.5".into()),
            }],
            references: vec![OsvReference {
                ref_type: "WEB".into(),
                url: "https://example.com/web-advisory".into(),
            }],
            affected: Vec::new(),
        };

        let vuln = convert_vulnerability(osv);
        assert_eq!(vuln.severity, Severity::High);
        assert_eq!(vuln.url, Some("https://example.com/web-advisory".into()));
    }
}
