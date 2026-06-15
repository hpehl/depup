//! Maven ecosystem support.
//!
//! Discovers version properties (`${version.*}`) across multi-module Maven projects,
//! resolves them against Maven Central and custom repositories defined in POMs,
//! and also resolves tool version properties (Node.js, npm, pnpm, yarn) against
//! their respective registries.

pub mod discovery;
pub mod maven_central;
pub mod node;
pub mod pm_versions;
pub mod pom;
pub mod pom_writer;
pub mod resolver;
pub mod tool;
pub mod updater;
