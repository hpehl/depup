use anyhow::Result;
use clap::ArgMatches;

use super::not_implemented;

#[allow(clippy::unnecessary_wraps)]
pub fn update(matches: &ArgMatches) -> Result<()> {
    not_implemented("update", matches)
}
