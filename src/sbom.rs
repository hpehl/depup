//! CycloneDX 1.5 SBOM generation from discovered dependencies.

use serde::Serialize;

use crate::model::{CheckResult, CommandResult, Ecosystem};

/// CycloneDX 1.5 Bill of Materials.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bom {
    pub bom_format: &'static str,
    pub spec_version: &'static str,
    pub version: u32,
    pub metadata: Metadata,
    pub components: Vec<Component>,
}

/// BOM metadata: tool identity and generation timestamp.
#[derive(Debug, Serialize)]
pub struct Metadata {
    pub timestamp: String,
    pub tools: Vec<Tool>,
}

/// Tool that generated the BOM.
#[derive(Debug, Serialize)]
pub struct Tool {
    pub name: &'static str,
    pub version: String,
}

/// A single dependency component in the BOM.
#[derive(Debug, Serialize)]
pub struct Component {
    #[serde(rename = "type")]
    pub component_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    pub name: String,
    pub version: String,
    pub purl: String,
}

/// Builds a CycloneDX 1.5 BOM from check results.
pub fn build_bom(results: &[CheckResult]) -> Bom {
    let components: Vec<Component> = results.iter().map(to_component).collect();
    Bom {
        bom_format: "CycloneDX",
        spec_version: "1.5",
        version: 1,
        metadata: Metadata {
            timestamp: iso8601_now(),
            tools: vec![Tool {
                name: "depup",
                version: env!("CARGO_PKG_VERSION").to_string(),
            }],
        },
        components,
    }
}

fn to_component(r: &CheckResult) -> Component {
    let (group, name) = split_artifact(r.artifact(), r.ecosystem());
    let purl = build_purl(r.ecosystem(), group.as_deref(), &name, &r.current_version);
    Component {
        component_type: "library",
        group,
        name,
        version: r.current_version.clone(),
        purl,
    }
}

fn split_artifact(artifact: &str, ecosystem: Ecosystem) -> (Option<String>, String) {
    match ecosystem {
        Ecosystem::Maven => {
            if let Some((g, a)) = artifact.split_once(':') {
                (Some(g.to_string()), a.to_string())
            } else {
                (None, artifact.to_string())
            }
        }
        Ecosystem::Npm => (None, artifact.to_string()),
    }
}

fn build_purl(ecosystem: Ecosystem, group: Option<&str>, name: &str, version: &str) -> String {
    match ecosystem {
        Ecosystem::Maven => {
            let g = group.unwrap_or("unknown");
            format!("pkg:maven/{g}/{name}@{version}")
        }
        Ecosystem::Npm => {
            if let Some(scoped) = name.strip_prefix('@') {
                let (scope, pkg) = scoped.split_once('/').unwrap_or((scoped, scoped));
                format!("pkg:npm/%40{scope}/{pkg}@{version}")
            } else {
                format!("pkg:npm/{name}@{version}")
            }
        }
    }
}

fn iso8601_now() -> String {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;

    // Simple date calculation from days since epoch
    let (year, month, day) = days_to_ymd(days);
    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    days += 719_468;
    let era = days / 146_097;
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Dependency, DependencyKind};

    fn maven_result(artifact: &str, version: &str) -> CheckResult {
        CheckResult::checked(
            Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                artifact.into(),
                None,
                "pom.xml".into(),
            ),
            version.into(),
            version.into(),
            false,
        )
    }

    fn npm_result(name: &str, version: &str) -> CheckResult {
        CheckResult::checked(
            Dependency::new(
                Ecosystem::Npm,
                DependencyKind::NpmDep,
                name.into(),
                None,
                "package.json".into(),
            ),
            version.into(),
            version.into(),
            false,
        )
    }

    #[test]
    fn purl_maven() {
        let r = maven_result("org.junit.jupiter:junit-jupiter", "5.12.0");
        let c = to_component(&r);
        assert_eq!(c.purl, "pkg:maven/org.junit.jupiter/junit-jupiter@5.12.0");
        assert_eq!(c.group, Some("org.junit.jupiter".into()));
        assert_eq!(c.name, "junit-jupiter");
    }

    #[test]
    fn purl_npm_unscoped() {
        let r = npm_result("react", "18.3.0");
        let c = to_component(&r);
        assert_eq!(c.purl, "pkg:npm/react@18.3.0");
        assert_eq!(c.group, None);
        assert_eq!(c.name, "react");
    }

    #[test]
    fn purl_npm_scoped() {
        let r = npm_result("@types/node", "20.0.0");
        let c = to_component(&r);
        assert_eq!(c.purl, "pkg:npm/%40types/node@20.0.0");
    }

    #[test]
    fn build_bom_structure() {
        let results = vec![
            maven_result("com.google:guava", "33.0.0"),
            npm_result("lodash", "4.17.21"),
        ];
        let bom = build_bom(&results);
        assert_eq!(bom.bom_format, "CycloneDX");
        assert_eq!(bom.spec_version, "1.5");
        assert_eq!(bom.version, 1);
        assert_eq!(bom.components.len(), 2);
        assert_eq!(bom.metadata.tools[0].name, "depup");
    }

    #[test]
    fn bom_serializes_to_valid_json() {
        let results = vec![maven_result("g:a", "1.0")];
        let bom = build_bom(&results);
        let json = serde_json::to_string_pretty(&bom).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["bomFormat"], "CycloneDX");
        assert_eq!(parsed["specVersion"], "1.5");
        assert_eq!(parsed["components"][0]["purl"], "pkg:maven/g/a@1.0");
    }

    #[test]
    fn empty_results_produce_empty_components() {
        let bom = build_bom(&[]);
        assert!(bom.components.is_empty());
    }

    #[test]
    fn days_to_ymd_epoch() {
        assert_eq!(days_to_ymd(0), (1970, 1, 1));
    }

    #[test]
    fn days_to_ymd_known_date() {
        // 2026-06-17 is day 20_621 since epoch
        assert_eq!(days_to_ymd(20_621), (2026, 6, 17));
    }
}
