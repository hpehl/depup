#!/usr/bin/env bash
# Creates one PR per dependency category for outdated dependencies.
# Called by action.yml with environment variables set from action inputs.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Resolve base branch
if [ -n "${DEPUP_BASE_BRANCH:-}" ]; then
  BASE_BRANCH="$DEPUP_BASE_BRANCH"
else
  BASE_BRANCH=$(gh repo view --json defaultBranchRef -q '.defaultBranchRef.name')
fi

# Build user filter flags from action inputs (as array for proper quoting)
USER_FLAGS=()
if [ "${DEPUP_STABLE:-false}" = "true" ]; then
  USER_FLAGS+=(--stable)
fi
if [ -n "${DEPUP_INCLUDE:-}" ]; then
  IFS=',' read -ra patterns <<< "$DEPUP_INCLUDE"
  for p in "${patterns[@]}"; do
    p=$(echo "$p" | xargs)
    [ -n "$p" ] && USER_FLAGS+=(--include "$p")
  done
fi
if [ -n "${DEPUP_EXCLUDE:-}" ]; then
  IFS=',' read -ra patterns <<< "$DEPUP_EXCLUDE"
  for p in "${patterns[@]}"; do
    p=$(echo "$p" | xargs)
    [ -n "$p" ] && USER_FLAGS+=(--exclude "$p")
  done
fi

# Build label flags for gh pr create (as array)
LABEL_FLAGS=()
if [ -n "${DEPUP_LABELS:-}" ]; then
  IFS=',' read -ra labels <<< "$DEPUP_LABELS"
  for l in "${labels[@]}"; do
    l=$(echo "$l" | xargs)
    [ -n "$l" ] && LABEL_FLAGS+=(--label "$l")
  done
fi

# Category definitions: "branch_suffix|category_flags|pr_label"
CATEGORIES=(
  "maven-managed-dependencies|--maven --dependencies --managed|Maven managed dependencies"
  "maven-unmanaged-dependencies|--maven --dependencies --unmanaged|Maven unmanaged dependencies"
  "maven-managed-plugins|--maven --plugins --managed|Maven managed plugins"
  "maven-unmanaged-plugins|--maven --plugins --unmanaged|Maven unmanaged plugins"
  "maven-tools|--maven --tools|Maven tool versions"
  "npm-tools|--npm --tools|npm packageManager versions"
)

FOUND_OUTDATED=0
PRS_CREATED=0
PRS_SKIPPED=0
CATEGORIES_EMPTY=0

for entry in "${CATEGORIES[@]}"; do
  IFS='|' read -r BRANCH_SUFFIX CATEGORY_FLAGS_STR PR_LABEL <<< "$entry"
  BRANCH="depup/${BRANCH_SUFFIX}"
  TITLE="chore(deps): bump ${PR_LABEL}"

  # Split category flags string into array
  read -ra CATEGORY_FLAGS <<< "$CATEGORY_FLAGS_STR"

  echo "::group::Category: ${PR_LABEL}"

  # Step 1: Check for outdated deps in this category
  CHECK_OUTPUT=$(mktemp)
  set +e
  depup check --json --outdated "${CATEGORY_FLAGS[@]}" "${USER_FLAGS[@]}" "${DEPUP_PATH}" > "$CHECK_OUTPUT" 2>&1
  CHECK_EXIT=$?
  set -e

  # Verify valid JSON array
  if ! jq -e 'type == "array"' "$CHECK_OUTPUT" > /dev/null 2>&1; then
    echo "::warning::depup check failed for ${PR_LABEL}"
    cat "$CHECK_OUTPUT"
    echo "::endgroup::"
    rm -f "$CHECK_OUTPUT"
    continue
  fi

  COUNT=$(jq 'length' "$CHECK_OUTPUT")
  if [ "$COUNT" -eq 0 ]; then
    echo "No outdated dependencies in this category."
    echo "::endgroup::"
    rm -f "$CHECK_OUTPUT"
    CATEGORIES_EMPTY=$((CATEGORIES_EMPTY + 1))
    continue
  fi

  FOUND_OUTDATED=1
  echo "Found ${COUNT} outdated dependencies."

  # Step 2: Check if a PR already exists for this branch
  EXISTING_PR=$(gh pr list --head "$BRANCH" --state open --json number -q '.[0].number' 2>/dev/null || echo "")
  if [ -n "$EXISTING_PR" ]; then
    echo "PR #${EXISTING_PR} already open for ${BRANCH}, skipping."
    echo "::endgroup::"
    rm -f "$CHECK_OUTPUT"
    PRS_SKIPPED=$((PRS_SKIPPED + 1))
    continue
  fi

  # Step 3: Create branch
  git checkout -b "$BRANCH" "$BASE_BRANCH"

  # Step 4: Run depup update with the same category + user flags
  set +e
  depup update "${CATEGORY_FLAGS[@]}" "${USER_FLAGS[@]}" "${DEPUP_PATH}" 2>&1
  UPDATE_EXIT=$?
  set -e

  if [ "$UPDATE_EXIT" -ne 0 ]; then
    echo "::warning::depup update failed for ${PR_LABEL} (exit ${UPDATE_EXIT})"
    git checkout "$BASE_BRANCH"
    git branch -D "$BRANCH" 2>/dev/null || true
    echo "::endgroup::"
    rm -f "$CHECK_OUTPUT"
    continue
  fi

  # Step 5: Check for actual changes
  if git diff --quiet; then
    echo "No file changes after update, skipping."
    git checkout "$BASE_BRANCH"
    git branch -D "$BRANCH" 2>/dev/null || true
    echo "::endgroup::"
    rm -f "$CHECK_OUTPUT"
    continue
  fi

  # Step 6: Build PR body
  PR_BODY=$(bash "$SCRIPT_DIR/build-pr-body.sh" "$CHECK_OUTPUT" "$PR_LABEL")

  # Step 7: Commit, push, create PR
  git add -A
  git commit -m "$TITLE"
  git push -u origin "$BRANCH"

  gh pr create \
    --base "$BASE_BRANCH" \
    --head "$BRANCH" \
    --title "$TITLE" \
    --body "$PR_BODY" \
    "${LABEL_FLAGS[@]}"

  PRS_CREATED=$((PRS_CREATED + 1))

  # Step 8: Reset to base branch
  git checkout "$BASE_BRANCH"
  git branch -D "$BRANCH" 2>/dev/null || true

  echo "::endgroup::"
  rm -f "$CHECK_OUTPUT"
done

# Summary
echo ""
echo "=== depup summary ==="
echo "PRs created: ${PRS_CREATED}"
echo "PRs skipped (already open): ${PRS_SKIPPED}"
echo "Categories with no outdated deps: ${CATEGORIES_EMPTY}"

# Set output
if [ "$FOUND_OUTDATED" -eq 1 ]; then
  echo "exit-code=1" >> "$GITHUB_OUTPUT"
else
  echo "exit-code=0" >> "$GITHUB_OUTPUT"
fi
