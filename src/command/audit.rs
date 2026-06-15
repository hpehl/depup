//! Stub for the `audit` subcommand (not yet implemented).

use anyhow::Result;
use clap::ArgMatches;

use super::not_implemented;

/// Placeholder for future dependency audit functionality.
#[allow(clippy::unnecessary_wraps)]
pub fn audit(matches: &ArgMatches) -> Result<()> {
    not_implemented("audit", matches)
}
