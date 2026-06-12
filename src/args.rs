use std::path::PathBuf;

use clap::ArgMatches;

pub fn path_argument(matches: &ArgMatches) -> PathBuf {
    matches
        .get_one::<String>("path")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn is_json(matches: &ArgMatches) -> bool {
    matches.get_flag("json")
}

pub fn is_outdated(matches: &ArgMatches) -> bool {
    matches.get_flag("outdated")
}

pub fn is_verbose(matches: &ArgMatches) -> bool {
    matches.get_flag("verbose")
}

pub fn include_pre_releases(matches: &ArgMatches) -> bool {
    matches.get_flag("include-pre-releases")
}
