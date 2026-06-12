use std::path::PathBuf;

use clap::ArgMatches;

pub fn path_argument(matches: &ArgMatches) -> PathBuf {
    matches
        .get_one::<String>("path")
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
}

pub fn is_json(matches: &ArgMatches) -> bool {
    matches.get_flag("json")
}

pub fn is_outdated(matches: &ArgMatches) -> bool {
    matches.get_flag("outdated")
}

pub fn releases_only(matches: &ArgMatches) -> bool {
    !matches.get_flag("include-pre-releases")
}
