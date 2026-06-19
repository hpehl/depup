#!/usr/bin/env bash
# Generates a PR body in Markdown from depup check JSON output.
# Usage: build-pr-body.sh <json-file> <category-label>

set -euo pipefail

JSON_FILE="${1:?Usage: build-pr-body.sh <json-file> <category-label>}"
CATEGORY_LABEL="${2:?}"

if [ ! -f "$JSON_FILE" ]; then
  echo "Bumps outdated ${CATEGORY_LABEL}."
  echo ""
  echo "---"
  echo "This PR was automatically created by [depup](https://github.com/hpehl/depup)."
  exit 0
fi

echo "Bumps outdated ${CATEGORY_LABEL}."
echo ""
echo "| Artifact | Current | Latest | Source |"
echo "|----------|---------|--------|--------|"

jq -r '.[] | "| `\(.artifact)` | \(.current) | \(.latest) | \(.source // "") |"' "$JSON_FILE"

echo ""
echo "---"
echo "This PR was automatically created by [depup](https://github.com/hpehl/depup)."
