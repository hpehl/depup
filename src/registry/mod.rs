pub mod maven;
pub mod node;
pub mod npm;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CheckerKind {
    Dependency,
    Plugin,
    Node,
    Npm,
}

impl std::fmt::Display for CheckerKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dependency => write!(f, "Dependency"),
            Self::Plugin => write!(f, "Plugin"),
            Self::Node => write!(f, "Node"),
            Self::Npm => write!(f, "npm"),
        }
    }
}

impl CheckerKind {
    pub fn color(&self) -> console::Style {
        match self {
            Self::Dependency => console::Style::new().cyan(),
            Self::Plugin => console::Style::new().magenta(),
            Self::Node => console::Style::new().green(),
            Self::Npm => console::Style::new().yellow(),
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Dependency => "\u{25a0}",
            Self::Plugin => "\u{25a0}",
            Self::Node => "\u{25a0}",
            Self::Npm => "\u{25a0}",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CheckResult {
    pub property_name: String,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub outdated: bool,
    pub skipped: bool,
    pub error: Option<String>,
    pub artifact: Option<String>,
    pub kind: CheckerKind,
}
