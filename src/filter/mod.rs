//! Post-check result filtering based on CLI flags.
//!
//! Filters are composable: ecosystem, kind, outdated, stable, managed,
//! include/exclude flags can be combined freely. A result must pass all
//! active filters. Include/exclude use glob-style wildcards (`*`) matched
//! against artifact names (Maven `groupId:artifactId`, npm package name).

mod glob;

use clap::ArgMatches;

use crate::model::{CommandResult, DependencyKind, Ecosystem, Severity, VersionResult};

/// Safely reads a boolean flag, returning `false` if the flag is not defined.
fn try_get_flag(matches: &ArgMatches, name: &str) -> bool {
    matches
        .try_get_one::<bool>(name)
        .ok()
        .flatten()
        .copied()
        .unwrap_or(false)
}

/// Which dependency kind(s) to include in the output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KindFilter {
    Dependencies,
    Plugins,
    DevDeps,
    Tools,
}

impl KindFilter {
    fn matches(self, kind: DependencyKind) -> bool {
        match self {
            Self::Dependencies => {
                matches!(kind, DependencyKind::Dependency | DependencyKind::NpmDep)
            }
            Self::Plugins => kind == DependencyKind::Plugin,
            Self::DevDeps => kind == DependencyKind::NpmDevDep,
            Self::Tools => kind == DependencyKind::Tool,
        }
    }
}

/// Composite filter built from CLI arguments.
///
/// All active criteria must match for a result to pass.
/// `None` values mean "no filter" for that dimension.
#[derive(Debug, Clone, Default)]
pub struct Filter {
    pub outdated: bool,
    pub stable: bool,
    pub managed: Option<bool>,
    pub ecosystem: Option<Ecosystem>,
    pub kind: Option<KindFilter>,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub severity: Option<Severity>,
}

impl Filter {
    /// Constructs a filter from the parsed CLI arguments.
    ///
    /// Safely handles flags that may not be defined for all subcommands
    /// (e.g., `outdated` is only on `check`, not `update`).
    pub fn from_matches(matches: &ArgMatches) -> Self {
        let managed = if try_get_flag(matches, "managed") {
            Some(true)
        } else if try_get_flag(matches, "unmanaged") {
            Some(false)
        } else {
            None
        };

        let ecosystem = if try_get_flag(matches, "maven") {
            Some(Ecosystem::Maven)
        } else if try_get_flag(matches, "npm") {
            Some(Ecosystem::Npm)
        } else {
            None
        };

        let kind = if try_get_flag(matches, "dependencies") {
            Some(KindFilter::Dependencies)
        } else if try_get_flag(matches, "plugins") {
            Some(KindFilter::Plugins)
        } else if try_get_flag(matches, "dev-deps") {
            Some(KindFilter::DevDeps)
        } else if try_get_flag(matches, "tools") {
            Some(KindFilter::Tools)
        } else {
            None
        };

        let severity = matches
            .try_get_one::<String>("severity")
            .ok()
            .flatten()
            .map(|s| Severity::from_str_label(s));

        let include: Vec<String> = matches
            .try_get_many::<String>("include")
            .ok()
            .flatten()
            .map(|vals| vals.cloned().collect())
            .unwrap_or_default();

        let exclude: Vec<String> = matches
            .try_get_many::<String>("exclude")
            .ok()
            .flatten()
            .map(|vals| vals.cloned().collect())
            .unwrap_or_default();

        Self {
            outdated: try_get_flag(matches, "outdated"),
            stable: try_get_flag(matches, "stable"),
            managed,
            ecosystem,
            kind,
            include,
            exclude,
            severity,
        }
    }

    /// Returns true if the given severity meets the minimum threshold.
    pub fn matches_severity(&self, severity: Severity) -> bool {
        match self.severity {
            Some(min) => severity >= min,
            None => true,
        }
    }

    /// Returns true if the result passes all active filter criteria.
    pub fn matches(&self, result: &VersionResult) -> bool {
        if self.outdated && !result.is_outdated() {
            return false;
        }
        if self.stable && result.is_skipped() {
            return false;
        }
        if let Some(managed) = self.managed
            && result.ecosystem() == Ecosystem::Maven
            && result.has_property() != managed
        {
            return false;
        }
        if let Some(ecosystem) = self.ecosystem
            && result.ecosystem() != ecosystem
        {
            return false;
        }
        if let Some(kind) = self.kind
            && !kind.matches(result.kind())
        {
            return false;
        }
        if !self.include.is_empty()
            && !self
                .include
                .iter()
                .any(|p| glob::glob_matches(p, result.artifact()))
        {
            return false;
        }
        if self
            .exclude
            .iter()
            .any(|p| glob::glob_matches(p, result.artifact()))
        {
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Dependency;

    fn maven_dep(has_prop: bool) -> VersionResult {
        let property = if has_prop {
            Some("version.junit".into())
        } else {
            None
        };
        VersionResult::checked(
            Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                "org.junit:junit".into(),
                property,
                String::new(),
            ),
            "5.10.0".into(),
            "5.12.0".into(),
            true,
        )
    }

    fn maven_plugin() -> VersionResult {
        VersionResult::checked(
            Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Plugin,
                "org.apache.maven.plugins:maven-compiler-plugin".into(),
                Some("version.compiler".into()),
                String::new(),
            ),
            "3.11.0".into(),
            "3.13.0".into(),
            true,
        )
    }

    fn npm_dep() -> VersionResult {
        VersionResult::checked(
            Dependency::new(
                Ecosystem::Npm,
                DependencyKind::NpmDep,
                "react".into(),
                None,
                String::new(),
            ),
            "18.0.0".into(),
            "19.0.0".into(),
            true,
        )
    }

    fn npm_dev_dep() -> VersionResult {
        VersionResult::checked(
            Dependency::new(
                Ecosystem::Npm,
                DependencyKind::NpmDevDep,
                "vitest".into(),
                None,
                String::new(),
            ),
            "1.0.0".into(),
            "2.0.0".into(),
            true,
        )
    }

    fn tool_version_result() -> VersionResult {
        VersionResult::checked(
            Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Tool,
                "nodejs.org".into(),
                None,
                String::new(),
            ),
            "20.0.0".into(),
            "22.0.0".into(),
            true,
        )
    }

    #[test]
    fn no_filters_passes_everything() {
        let f = Filter::default();
        assert!(f.matches(&maven_dep(true)));
        assert!(f.matches(&maven_dep(false)));
        assert!(f.matches(&maven_plugin()));
        assert!(f.matches(&npm_dep()));
        assert!(f.matches(&npm_dev_dep()));
        assert!(f.matches(&tool_version_result()));
    }

    #[test]
    fn outdated_filter() {
        let f = Filter {
            outdated: true,
            ..Filter::default()
        };
        assert!(f.matches(&maven_dep(true)));

        let up_to_date = VersionResult::checked(
            Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                "p".into(),
                None,
                String::new(),
            ),
            "1.0".into(),
            "1.0".into(),
            false,
        );
        assert!(!f.matches(&up_to_date));
    }

    #[test]
    fn stable_filter_excludes_skipped() {
        let f = Filter {
            stable: true,
            ..Filter::default()
        };
        let skipped = VersionResult::skipped(
            Dependency::new(
                Ecosystem::Maven,
                DependencyKind::Dependency,
                "p".into(),
                None,
                String::new(),
            ),
            "1.0-alpha".into(),
        );
        assert!(!f.matches(&skipped));
        assert!(f.matches(&maven_dep(true)));
    }

    #[test]
    fn managed_filter() {
        let f = Filter {
            managed: Some(true),
            ..Filter::default()
        };
        assert!(f.matches(&maven_dep(true)));
        assert!(!f.matches(&maven_dep(false)));
        assert!(f.matches(&npm_dep()));
    }

    #[test]
    fn unmanaged_filter() {
        let f = Filter {
            managed: Some(false),
            ..Filter::default()
        };
        assert!(!f.matches(&maven_dep(true)));
        assert!(f.matches(&maven_dep(false)));
        assert!(f.matches(&npm_dep()));
    }

    #[test]
    fn ecosystem_filter() {
        let maven_only = Filter {
            ecosystem: Some(Ecosystem::Maven),
            ..Filter::default()
        };
        assert!(maven_only.matches(&maven_dep(true)));
        assert!(!maven_only.matches(&npm_dep()));

        let npm_only = Filter {
            ecosystem: Some(Ecosystem::Npm),
            ..Filter::default()
        };
        assert!(!npm_only.matches(&maven_dep(true)));
        assert!(npm_only.matches(&npm_dep()));
    }

    #[test]
    fn kind_filter_dependencies() {
        let f = Filter {
            kind: Some(KindFilter::Dependencies),
            ..Filter::default()
        };
        assert!(f.matches(&maven_dep(true)));
        assert!(f.matches(&npm_dep()));
        assert!(!f.matches(&maven_plugin()));
        assert!(!f.matches(&npm_dev_dep()));
        assert!(!f.matches(&tool_version_result()));
    }

    #[test]
    fn kind_filter_plugins() {
        let f = Filter {
            kind: Some(KindFilter::Plugins),
            ..Filter::default()
        };
        assert!(f.matches(&maven_plugin()));
        assert!(!f.matches(&maven_dep(true)));
        assert!(!f.matches(&npm_dep()));
    }

    #[test]
    fn kind_filter_dev_deps() {
        let f = Filter {
            kind: Some(KindFilter::DevDeps),
            ..Filter::default()
        };
        assert!(f.matches(&npm_dev_dep()));
        assert!(!f.matches(&npm_dep()));
        assert!(!f.matches(&maven_dep(true)));
    }

    #[test]
    fn kind_filter_tool_versions() {
        let f = Filter {
            kind: Some(KindFilter::Tools),
            ..Filter::default()
        };
        assert!(f.matches(&tool_version_result()));
        assert!(!f.matches(&maven_dep(true)));
        assert!(!f.matches(&npm_dep()));
    }

    #[test]
    fn composable_filters() {
        let f = Filter {
            ecosystem: Some(Ecosystem::Maven),
            kind: Some(KindFilter::Dependencies),
            managed: Some(true),
            outdated: true,
            stable: false,
            ..Filter::default()
        };
        assert!(f.matches(&maven_dep(true)));
        assert!(!f.matches(&maven_dep(false)));
        assert!(!f.matches(&maven_plugin()));
        assert!(!f.matches(&npm_dep()));
    }

    // ------------------------------------------------------------------
    // Include / exclude filters
    // ------------------------------------------------------------------

    #[test]
    fn include_filter() {
        let f = Filter {
            include: vec!["org.junit:*".into()],
            ..Filter::default()
        };
        assert!(f.matches(&maven_dep(true)));
        assert!(!f.matches(&maven_plugin()));
        assert!(!f.matches(&npm_dep()));
    }

    #[test]
    fn include_multiple_patterns() {
        let f = Filter {
            include: vec!["org.junit:*".into(), "react".into()],
            ..Filter::default()
        };
        assert!(f.matches(&maven_dep(true)));
        assert!(f.matches(&npm_dep()));
        assert!(!f.matches(&maven_plugin()));
    }

    #[test]
    fn exclude_filter() {
        let f = Filter {
            exclude: vec!["org.junit:*".into()],
            ..Filter::default()
        };
        assert!(!f.matches(&maven_dep(true)));
        assert!(f.matches(&maven_plugin()));
        assert!(f.matches(&npm_dep()));
    }

    #[test]
    fn include_then_exclude() {
        let f = Filter {
            include: vec!["org.*:*".into()],
            exclude: vec!["*:junit".into()],
            ..Filter::default()
        };
        // org.junit:junit matches include but also matches exclude
        assert!(!f.matches(&maven_dep(true)));
        // org.apache.maven.plugins:maven-compiler-plugin matches include, not excluded
        assert!(f.matches(&maven_plugin()));
        // npm dep doesn't match include at all
        assert!(!f.matches(&npm_dep()));
    }
}
