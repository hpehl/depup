#!/usr/bin/env bash
# Detects npm ecosystem projects and installs missing package managers.
# Called by action.yml before create-prs.sh.
# Relies on: DEPUP_PATH env var (defaults to .), jq, node/npm on PATH.

set -euo pipefail

PROJECT_ROOT="${DEPUP_PATH:-.}"

NEED_PNPM=0
NEED_BUN=0
PNPM_COREPACK_DIR=""

echo "::group::Detecting package managers"

# Find all package.json files, skipping irrelevant directories
while IFS= read -r pkg_json; do
  dir=$(dirname "$pkg_json")

  # Detect by lock file
  if [ -f "$dir/pnpm-lock.yaml" ]; then
    NEED_PNPM=1
  fi
  # yarn.lock → yarn classic is pre-installed, nothing to do
  # package-lock.json → npm is pre-installed, nothing to do
  if [ -f "$dir/bun.lock" ] || [ -f "$dir/bun.lockb" ]; then
    NEED_BUN=1
  fi

  # Detect by packageManager field
  if [ -f "$pkg_json" ]; then
    PM_NAME=$(jq -r '.packageManager // empty' "$pkg_json" 2>/dev/null | cut -d'@' -f1)
    case "$PM_NAME" in
      pnpm)
        NEED_PNPM=1
        # Remember a directory with packageManager for corepack
        if [ -z "$PNPM_COREPACK_DIR" ]; then
          PNPM_COREPACK_DIR="$dir"
        fi
        ;;
      bun) NEED_BUN=1 ;;
      # npm and yarn classic are pre-installed
    esac
  fi
done < <(find "$PROJECT_ROOT" -name package.json \
  -not -path '*/node_modules/*' \
  -not -path '*/.pnpm-store/*' \
  -not -path '*/.yarn/*' \
  -not -path '*/.bun/*' \
  -not -path '*/.git/*' \
  -not -path '*/target/*' \
  -not -path '*/dist/*' \
  -not -path '*/build/*')

echo "Detected: pnpm=$NEED_PNPM bun=$NEED_BUN"
echo "::endgroup::"

# Install pnpm if needed and missing
if [ "$NEED_PNPM" -eq 1 ] && ! command -v pnpm &>/dev/null; then
  echo "::group::Installing pnpm"
  if [ -n "$PNPM_COREPACK_DIR" ]; then
    corepack enable
    (cd "$PNPM_COREPACK_DIR" && corepack install)
    echo "Installed pnpm via corepack (from $PNPM_COREPACK_DIR/package.json)"
  else
    npm install -g pnpm
    echo "Installed pnpm via npm"
  fi
  echo "::endgroup::"
fi

# Install bun if needed and missing
if [ "$NEED_BUN" -eq 1 ] && ! command -v bun &>/dev/null; then
  echo "::group::Installing bun"
  npm install -g bun
  echo "Installed bun via npm"
  echo "::endgroup::"
fi
