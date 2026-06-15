use anyhow::Result;
use clap::ArgMatches;

use super::not_implemented;

#[allow(clippy::unnecessary_wraps)]
pub fn audit(matches: &ArgMatches) -> Result<()> {
    not_implemented("audit", matches)
}
