//! Stub for the `update` subcommand (not yet implemented).

use anyhow::Result;
use clap::ArgMatches;

use super::not_implemented;

/// Placeholder for future dependency update functionality.
#[allow(clippy::unnecessary_wraps)]
pub fn update(matches: &ArgMatches) -> Result<()> {
    not_implemented("update", matches)
}
