//! Version parsing and comparison.
//!
//! Handles Maven-specific formats like `3.0.0.Final` and `2.1.0-SP1` that
//! don't follow strict semver. Qualifiers are separated by `-` or the first
//! alphabetic character after the numeric prefix. A version without a qualifier
//! is considered newer than the same numeric version with a qualifier
//! (e.g., `1.0.0` > `1.0.0.Final`).

use std::cmp::Ordering;
use std::fmt;

/// A parsed version with numeric components and an optional qualifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub qualifier: Option<String>,
    pub raw: String,
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

impl Version {
    /// Parses a version string into its numeric and qualifier components.
    /// Returns `None` if the string is empty or has no numeric prefix.
    pub fn parse(raw: &str) -> Option<Self> {
        if raw.is_empty() {
            return None;
        }
        let stripped = raw.strip_prefix('v').unwrap_or(raw);
        let (numeric_part, qualifier) = split_qualifier(stripped);
        if numeric_part.is_empty() {
            return None;
        }
        let parts: Vec<&str> = numeric_part.split('.').collect();

        let major = parts.first()?.parse().ok()?;
        let minor = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        let patch = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);

        Some(Self {
            major,
            minor,
            patch,
            qualifier,
            raw: raw.to_string(),
        })
    }

    /// Returns true if the qualifier indicates a pre-release version
    /// (alpha, beta, RC, CR, snapshot, milestone, preview, dev, incubating).
    pub fn is_pre_release(&self) -> bool {
        self.qualifier
            .as_ref()
            .is_some_and(|q| is_pre_release_qualifier(&q.to_lowercase()))
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        let numeric = self
            .major
            .cmp(&other.major)
            .then(self.minor.cmp(&other.minor))
            .then(self.patch.cmp(&other.patch));

        if numeric != Ordering::Equal {
            return numeric;
        }

        match (&self.qualifier, &other.qualifier) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(a), Some(b)) => a.to_lowercase().cmp(&b.to_lowercase()),
        }
    }
}

/// Splits a version string into numeric and qualifier parts.
/// Handles both `-` separators (`2.1.0-SP1`) and dot-joined qualifiers (`3.0.0.Final`).
fn split_qualifier(version: &str) -> (&str, Option<String>) {
    if version.is_empty() {
        return ("", None);
    }
    if let Some(pos) = version.find('-') {
        (&version[..pos], Some(version[pos + 1..].to_string()))
    } else if let Some(pos) = version.find(|c: char| c.is_ascii_alphabetic()) {
        if pos == 0 {
            return ("", Some(version.to_string()));
        }
        if version[..pos].ends_with('.') {
            (&version[..pos - 1], Some(version[pos..].to_string()))
        } else {
            (&version[..pos], Some(version[pos..].to_string()))
        }
    } else {
        (version, None)
    }
}

/// Checks if a lowercased qualifier string indicates a pre-release.
/// Also matches milestone-style qualifiers like `M1`, `M2`, etc.
fn is_pre_release_qualifier(lower: &str) -> bool {
    let patterns = [
        "alpha",
        "beta",
        "rc",
        "cr",
        "snapshot",
        "milestone",
        "preview",
        "dev",
        "incubating",
    ];
    patterns.iter().any(|p| lower.contains(p))
        || (lower.starts_with('m')
            && lower.len() > 1
            && lower[1..].chars().all(|c| c.is_ascii_digit()))
}

/// Finds the highest version from a list of version strings.
#[must_use]
pub fn find_latest(versions: &[String]) -> Option<String> {
    let mut parsed: Vec<_> = versions.iter().filter_map(|v| Version::parse(v)).collect();
    parsed.sort();
    parsed.last().map(|v| v.raw.clone())
}

/// Returns true if `latest` is a newer version than `current`.
///
/// Falls back to string inequality when either version cannot be parsed.
/// This is intentionally conservative: unparseable versions are treated as
/// "different means outdated", which is the safer default for alerting users.
#[must_use]
pub fn is_newer(current: &str, latest: &str) -> bool {
    match (Version::parse(current), Version::parse(latest)) {
        (Some(c), Some(l)) => l > c,
        _ => latest != current,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_semver() {
        let v = Version::parse("1.2.3").unwrap();
        assert_eq!((v.major, v.minor, v.patch), (1, 2, 3));
        assert_eq!(v.qualifier, None);
    }

    #[test]
    fn parse_two_part() {
        let v = Version::parse("3.14").unwrap();
        assert_eq!((v.major, v.minor, v.patch), (3, 14, 0));
    }

    #[test]
    fn parse_single_number() {
        let v = Version::parse("42").unwrap();
        assert_eq!((v.major, v.minor, v.patch), (42, 0, 0));
        assert_eq!(v.qualifier, None);
    }

    #[test]
    fn parse_with_qualifier() {
        let v = Version::parse("3.0.0.Final").unwrap();
        assert_eq!((v.major, v.minor, v.patch), (3, 0, 0));
        assert_eq!(v.qualifier.as_deref(), Some("Final"));
    }

    #[test]
    fn parse_with_dash_qualifier() {
        let v = Version::parse("2.1.0-SP1").unwrap();
        assert_eq!(v.qualifier.as_deref(), Some("SP1"));
    }

    #[test]
    fn parse_v_prefix() {
        let v = Version::parse("v26.3.0").unwrap();
        assert_eq!((v.major, v.minor, v.patch), (26, 3, 0));
    }

    #[test]
    fn parse_empty_returns_none() {
        assert!(Version::parse("").is_none());
    }

    #[test]
    fn parse_all_alpha_returns_none() {
        assert!(Version::parse("Final").is_none());
    }

    #[test]
    fn parse_trailing_dot() {
        let v = Version::parse("1.0.0.").unwrap();
        assert_eq!((v.major, v.minor, v.patch), (1, 0, 0));
    }

    #[test]
    fn pre_release_detection() {
        assert!(Version::parse("1.0.0-alpha1").unwrap().is_pre_release());
        assert!(Version::parse("1.0.0-beta2").unwrap().is_pre_release());
        assert!(Version::parse("1.0.0-RC1").unwrap().is_pre_release());
        assert!(Version::parse("1.0.0-M3").unwrap().is_pre_release());
        assert!(Version::parse("1.0.0-SNAPSHOT").unwrap().is_pre_release());
        assert!(!Version::parse("1.0.0.Final").unwrap().is_pre_release());
        assert!(!Version::parse("1.0.0").unwrap().is_pre_release());
        assert!(!Version::parse("1.0.0-SP1").unwrap().is_pre_release());
    }

    #[test]
    fn bare_m_is_not_pre_release() {
        assert!(!Version::parse("1.0.0-m").unwrap().is_pre_release());
    }

    #[test]
    fn ordering() {
        let v1 = Version::parse("1.0.0").unwrap();
        let v2 = Version::parse("2.0.0").unwrap();
        assert!(v2 > v1);

        let v3 = Version::parse("1.0.0.Final").unwrap();
        let v4 = Version::parse("1.0.0").unwrap();
        assert!(v4 > v3);

        let v5 = Version::parse("3.0.0.Final").unwrap();
        let v6 = Version::parse("3.1.0.Final").unwrap();
        assert!(v6 > v5);
    }

    #[test]
    fn is_newer_works() {
        assert!(is_newer("1.0.0", "2.0.0"));
        assert!(!is_newer("2.0.0", "1.0.0"));
        assert!(!is_newer("1.0.0", "1.0.0"));
    }

    #[test]
    fn is_newer_unparseable_falls_back_to_string_compare() {
        assert!(is_newer("abc", "def"));
        assert!(!is_newer("abc", "abc"));
    }

    #[test]
    fn find_latest_returns_highest() {
        let versions = vec![
            "1.0.0".to_string(),
            "2.3.1".to_string(),
            "2.1.0".to_string(),
        ];
        assert_eq!(find_latest(&versions), Some("2.3.1".to_string()));
    }

    #[test]
    fn find_latest_with_qualifiers() {
        let versions = vec![
            "3.0.0.Final".to_string(),
            "3.1.0.Final".to_string(),
            "2.5.0.Final".to_string(),
        ];
        assert_eq!(find_latest(&versions), Some("3.1.0.Final".to_string()));
    }

    #[test]
    fn find_latest_empty_returns_none() {
        let versions: Vec<String> = vec![];
        assert_eq!(find_latest(&versions), None);
    }
}
