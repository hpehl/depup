//! Post-check result filtering based on CLI flags.
//!
//! Filters are composable: ecosystem, kind, outdated, stable, and managed
//! flags can be combined freely. A result must pass all active filters.

use clap::ArgMatches;

use crate::registry::{CheckResult, CheckerKind, Ecosystem};

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
    ToolVersions,
}

impl KindFilter {
    fn matches(self, kind: CheckerKind) -> bool {
        match self {
            Self::Dependencies => matches!(kind, CheckerKind::Dependency | CheckerKind::NpmDep),
            Self::Plugins => kind == CheckerKind::Plugin,
            Self::DevDeps => kind == CheckerKind::NpmDevDep,
            Self::ToolVersions => kind == CheckerKind::ToolVersion,
        }
    }
}

/// Composite filter built from CLI arguments.
///
/// All active criteria must match for a result to pass.
/// `None` values mean "no filter" for that dimension.
#[derive(Debug, Clone)]
pub struct Filter {
    pub outdated: bool,
    pub stable: bool,
    pub managed: Option<bool>,
    pub ecosystem: Option<Ecosystem>,
    pub kind: Option<KindFilter>,
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
            Some(KindFilter::ToolVersions)
        } else {
            None
        };

        Self {
            outdated: try_get_flag(matches, "outdated"),
            stable: try_get_flag(matches, "stable"),
            managed,
            ecosystem,
            kind,
        }
    }

    /// Returns true if the result passes all active filter criteria.
    pub fn matches(&self, result: &CheckResult) -> bool {
        if self.outdated && !result.is_outdated() {
            return false;
        }
        if self.stable && result.is_skipped() {
            return false;
        }
        if let Some(managed) = self.managed
            && result.ecosystem() == Ecosystem::Maven
            && result.has_version_property() != managed
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
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::CheckId;

    fn maven_dep(has_prop: bool) -> CheckResult {
        CheckResult::checked(
            CheckId::new(
                Ecosystem::Maven,
                CheckerKind::Dependency,
                "version.junit".into(),
                None,
                String::new(),
            )
            .with_version_property(has_prop),
            "5.10.0".into(),
            "5.12.0".into(),
            true,
        )
    }

    fn maven_plugin() -> CheckResult {
        CheckResult::checked(
            CheckId::new(
                Ecosystem::Maven,
                CheckerKind::Plugin,
                "version.compiler".into(),
                None,
                String::new(),
            ),
            "3.11.0".into(),
            "3.13.0".into(),
            true,
        )
    }

    fn npm_dep() -> CheckResult {
        CheckResult::checked(
            CheckId::new(
                Ecosystem::Npm,
                CheckerKind::NpmDep,
                "react".into(),
                Some("react".into()),
                String::new(),
            ),
            "18.0.0".into(),
            "19.0.0".into(),
            true,
        )
    }

    fn npm_dev_dep() -> CheckResult {
        CheckResult::checked(
            CheckId::new(
                Ecosystem::Npm,
                CheckerKind::NpmDevDep,
                "vitest".into(),
                Some("vitest".into()),
                String::new(),
            ),
            "1.0.0".into(),
            "2.0.0".into(),
            true,
        )
    }

    fn tool_version_result() -> CheckResult {
        CheckResult::checked(
            CheckId::new(
                Ecosystem::Maven,
                CheckerKind::ToolVersion,
                "version.node".into(),
                None,
                String::new(),
            ),
            "20.0.0".into(),
            "22.0.0".into(),
            true,
        )
    }

    fn no_filter() -> Filter {
        Filter {
            outdated: false,
            stable: false,
            managed: None,
            ecosystem: None,
            kind: None,
        }
    }

    #[test]
    fn no_filters_passes_everything() {
        let f = no_filter();
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
            ..no_filter()
        };
        assert!(f.matches(&maven_dep(true)));

        let up_to_date = CheckResult::checked(
            CheckId::new(
                Ecosystem::Maven,
                CheckerKind::Dependency,
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
            ..no_filter()
        };
        let skipped = CheckResult::skipped(
            CheckId::new(
                Ecosystem::Maven,
                CheckerKind::Dependency,
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
            ..no_filter()
        };
        assert!(f.matches(&maven_dep(true)));
        assert!(!f.matches(&maven_dep(false)));
        assert!(f.matches(&npm_dep()));
    }

    #[test]
    fn unmanaged_filter() {
        let f = Filter {
            managed: Some(false),
            ..no_filter()
        };
        assert!(!f.matches(&maven_dep(true)));
        assert!(f.matches(&maven_dep(false)));
        assert!(f.matches(&npm_dep()));
    }

    #[test]
    fn ecosystem_filter() {
        let maven_only = Filter {
            ecosystem: Some(Ecosystem::Maven),
            ..no_filter()
        };
        assert!(maven_only.matches(&maven_dep(true)));
        assert!(!maven_only.matches(&npm_dep()));

        let npm_only = Filter {
            ecosystem: Some(Ecosystem::Npm),
            ..no_filter()
        };
        assert!(!npm_only.matches(&maven_dep(true)));
        assert!(npm_only.matches(&npm_dep()));
    }

    #[test]
    fn kind_filter_dependencies() {
        let f = Filter {
            kind: Some(KindFilter::Dependencies),
            ..no_filter()
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
            ..no_filter()
        };
        assert!(f.matches(&maven_plugin()));
        assert!(!f.matches(&maven_dep(true)));
        assert!(!f.matches(&npm_dep()));
    }

    #[test]
    fn kind_filter_dev_deps() {
        let f = Filter {
            kind: Some(KindFilter::DevDeps),
            ..no_filter()
        };
        assert!(f.matches(&npm_dev_dep()));
        assert!(!f.matches(&npm_dep()));
        assert!(!f.matches(&maven_dep(true)));
    }

    #[test]
    fn kind_filter_tool_versions() {
        let f = Filter {
            kind: Some(KindFilter::ToolVersions),
            ..no_filter()
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
        };
        assert!(f.matches(&maven_dep(true)));
        assert!(!f.matches(&maven_dep(false)));
        assert!(!f.matches(&maven_plugin()));
        assert!(!f.matches(&npm_dep()));
    }
}
