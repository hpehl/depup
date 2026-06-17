#!/usr/bin/env bash
# Formats depup JSON output as a Markdown report for GitHub Actions job summaries.
# Usage: format-report.sh <json-file> <command> <exit-code>

set -euo pipefail

JSON_FILE="${1:?Usage: format-report.sh <json-file> <command> <exit-code>}"
COMMAND="${2:?}"
EXIT_CODE="${3:?}"

if [ ! -f "$JSON_FILE" ]; then
  echo "## depup ${COMMAND}"
  echo ""
  echo "> No output file found."
  exit 0
fi

# Check if output is valid JSON array
if ! jq -e 'type == "array"' "$JSON_FILE" > /dev/null 2>&1; then
  echo "## depup ${COMMAND}"
  echo ""
  echo "> depup returned an error:"
  echo '```'
  cat "$JSON_FILE"
  echo '```'
  exit 0
fi

COUNT=$(jq 'length' "$JSON_FILE")

case "$COMMAND" in
  check)
    format_check() {
      OUTDATED=$(jq '[.[] | select(.status == "outdated")] | length' "$JSON_FILE")
      if [ "$OUTDATED" -eq 0 ]; then
        echo "## :white_check_mark: depup check — all dependencies up to date"
        echo ""
        echo "Checked **${COUNT}** dependencies."
      else
        echo "## :warning: depup check — ${OUTDATED} outdated dependencies"
        echo ""
        echo "| Ecosystem | Artifact | Current | Latest | Kind |"
        echo "|-----------|----------|---------|--------|------|"
        jq -r '.[] | select(.status == "outdated") | "| \(.ecosystem) | `\(.artifact)` | \(.current) | \(.latest) | \(.kind) |"' "$JSON_FILE"
      fi
    }
    format_check
    ;;

  audit)
    format_audit() {
      VULNERABLE=$(jq '[.[] | select(.vulnerable == true)] | length' "$JSON_FILE")
      if [ "$VULNERABLE" -eq 0 ]; then
        echo "## :white_check_mark: depup audit — no vulnerabilities found"
        echo ""
        echo "Audited **${COUNT}** dependencies."
      else
        VULN_COUNT=$(jq '[.[] | select(.vulnerable == true) | .vulnerabilities[]] | length' "$JSON_FILE")
        SEVERITY_LABEL=""
        if [ "$EXIT_CODE" -eq 3 ]; then
          SEVERITY_LABEL=" (includes critical/high)"
        fi
        echo "## :rotating_light: depup audit — ${VULN_COUNT} vulnerabilities in ${VULNERABLE} dependencies${SEVERITY_LABEL}"
        echo ""
        echo "| Ecosystem | Artifact | Version | Vuln ID | Severity | Summary |"
        echo "|-----------|----------|---------|---------|----------|---------|"
        jq -r '.[] | select(.vulnerable == true) | . as $dep | .vulnerabilities[] | "| \($dep.ecosystem) | `\($dep.artifact)` | \($dep.version) | \(.id) | \(.severity) | \(.summary) |"' "$JSON_FILE"
      fi
    }
    format_audit
    ;;

  sbom)
    echo "## :package: depup sbom"
    echo ""
    # SBOM output is a single object, not an array
    if jq -e '.components' "$JSON_FILE" > /dev/null 2>&1; then
      COMP_COUNT=$(jq '.components | length' "$JSON_FILE")
      echo "Generated CycloneDX 1.5 SBOM with **${COMP_COUNT}** components."
    else
      echo "SBOM generated. See the artifact for the full output."
    fi
    ;;

  *)
    echo "## depup ${COMMAND}"
    echo ""
    echo "Results: **${COUNT}** items."
    ;;
esac
