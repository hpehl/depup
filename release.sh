#!/usr/bin/env bash

# -------------------------------------------------------
#
# Creates a new release using cargo-release:
#   - bump version in Cargo.toml
#   - update CHANGELOG.md (Unreleased → version)
#   - run cargo fmt
#   - commit & push changes
#   - create & push tag (triggers GitHub release workflow)
#
# -------------------------------------------------------

set -Eeuo pipefail
trap cleanup SIGINT SIGTERM ERR EXIT

VERSION=0.0.1

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" &>/dev/null && pwd -P)
readonly script_dir
cd "${script_dir}"

usage() {
  cat <<EOF
USAGE:
    $(basename "${BASH_SOURCE[0]}") [FLAGS] <release-version>

FLAGS:
    -h, --help          Prints help information
    -v, --version       Prints version information
    --no-color          Uses plain text output

ARGS:
    <release-version>   The release version (as semver)
EOF
  exit
}

cleanup() {
  trap - SIGINT SIGTERM ERR EXIT
}

setup_colors() {
  if [[ -t 2 ]] && [[ -z "${NO_COLOR-}" ]] && [[ "${TERM-}" != "dumb" ]]; then
    NOFORMAT='\033[0m' RED='\033[0;31m' GREEN='\033[0;32m' ORANGE='\033[0;33m' BLUE='\033[0;34m' PURPLE='\033[0;35m' CYAN='\033[0;36m' YELLOW='\033[1;33m'
  else
    # shellcheck disable=SC2034
    NOFORMAT='' RED='' GREEN='' ORANGE='' BLUE='' PURPLE='' CYAN='' YELLOW=''
  fi
}

msg() {
  echo >&2 -e "${1-}"
}

die() {
  local msg=$1
  local code=${2-1}
  msg "$msg"
  exit "$code"
}

version() {
  msg "${BASH_SOURCE[0]} $VERSION"
  exit 0
}

parse_params() {
  while :; do
    case "${1-}" in
    -h | --help) usage ;;
    -v | --version) version ;;
    --no-color) NO_COLOR=1 ;;
    -?*) die "Unknown option: $1" ;;
    *) break ;;
    esac
    shift
  done

  ARGS=("$@")
  [[ ${#ARGS[@]} -eq 1 ]] || die "Missing release version"
  RELEASE_VERSION=${ARGS[0]}
  return 0
}

is_semver() {
    local version
    version="$1"
    if [[ ! ${version} =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        return 1
    fi
}

parse_params "$@"
setup_colors

TAG="v${RELEASE_VERSION}"

is_semver "${RELEASE_VERSION}" || die "Release version is not a semantic version"
git diff-index --quiet HEAD || die "You have uncommitted changes"
[[ $(git tag -l "${TAG}") ]] && die "Tag ${TAG} already defined"

msg ""
msg "Codebase is ready to be released."
msg ""
msg "If you decide to continue, this script will "
msg ""
msg "   1. Bump the version to ${CYAN}${RELEASE_VERSION}${NOFORMAT}"
msg "   2. Create a tag for ${CYAN}${TAG}${NOFORMAT}"
msg "   3. ${CYAN}Commit${NOFORMAT} and ${CYAN}push${NOFORMAT} to origin (which will trigger the ${CYAN}release workflow${NOFORMAT} at GitHub)"
msg ""
echo "Do you wish to continue?"
select yn in "Yes" "No"; do
    case $yn in
        Yes ) break;;
        No ) die "Aborted ";;
    esac
done

msg ""
cargo release "${RELEASE_VERSION}" --no-confirm --execute
msg "Done. Watch the release workflow at https://github.com/hpehl/depup/actions/workflows/release.yml"
