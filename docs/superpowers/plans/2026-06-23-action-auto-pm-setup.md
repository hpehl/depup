# Auto Package Manager Setup in GitHub Action

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `hpehl/depup@v2` automatically detect and install needed package managers so users never have to add `pnpm/action-setup`, `actions/setup-node`, or `oven-sh/setup-bun` steps themselves.

**Architecture:** A new shell script `action-scripts/setup-pm.sh` scans the project tree for lock files and `packageManager` fields, then installs any missing package managers. The action.yml composite action calls this script as a step before `create-prs.sh`. The detection logic mirrors depup's own discovery (same lock files, same `packageManager` field parsing) but is implemented in bash since the action runs before depup is available.

**Tech Stack:** Bash, `jq` (pre-installed on GitHub runners), `corepack` (ships with Node.js on runners), `npm install -g` for fallbacks.

## Global Constraints

- GitHub `ubuntu-latest` runners have Node.js 22.x, npm 10.x, and yarn 1.x pre-installed. pnpm and bun are **not** pre-installed.
- `corepack` ships with Node.js but is disabled by default — must run `corepack enable` first.
- `corepack` can install pnpm and yarn when `packageManager` is set in `package.json`, but it reads from the **current directory's** `package.json` only.
- Detection must match depup's own logic: lock files (`pnpm-lock.yaml`, `package-lock.json`, `yarn.lock`, `bun.lock`/`bun.lockb`) and `packageManager` field in `package.json`.
- Skip the same directories depup skips: `node_modules`, `.pnpm-store`, `.yarn`, `.bun`, `.git`, `target`, `dist`, `build`.
- The script must be idempotent — safe to run when PMs are already installed.
- No changes to the depup Rust binary.
- Action must continue to work for Maven-only projects (no npm detection = no PM install = no error).

---

### Task 1: Create `setup-pm.sh` — detect and install package managers

**Files:**
- Create: `action-scripts/setup-pm.sh`

**Interfaces:**
- Consumes: `DEPUP_PATH` environment variable (project root, defaults to `.`)
- Produces: pnpm, yarn, and/or bun available on `PATH` when the script exits

**Detection algorithm:**

1. Walk the directory tree under `DEPUP_PATH`, skipping `node_modules`, `.pnpm-store`, `.yarn`, `.bun`, `.git`, `target`, `dist`, `build`.
2. For each `package.json` found, check for adjacent lock files and the `packageManager` field.
3. Collect a deduplicated set of needed package managers: `pnpm`, `yarn`, `bun`. (npm is always pre-installed on runners, so skip it.)
4. For each needed PM, check if it's already on PATH. If not, install it.

**Installation strategy per PM:**

| PM | Install method | Why |
|----|---------------|-----|
| pnpm | `corepack enable && corepack install` in a dir with `packageManager` field, or `npm install -g pnpm` as fallback | corepack respects the project's pinned version; npm fallback for projects without `packageManager` |
| yarn | Already on runners (yarn 1.x). For yarn berry (v2+), `corepack enable && corepack install` | yarn classic is pre-installed; berry needs corepack |
| bun | `npm install -g bun` | No corepack support for bun; npm is the simplest install path |

- [ ] **Step 1: Create `action-scripts/setup-pm.sh` with detection logic**

```bash
#!/usr/bin/env bash
# Detects npm ecosystem projects and installs missing package managers.
# Called by action.yml before create-prs.sh.
# Relies on: DEPUP_PATH env var (defaults to .), jq, node/npm on PATH.

set -euo pipefail

PROJECT_ROOT="${DEPUP_PATH:-.}"

SKIP_DIRS="node_modules|\.pnpm-store|\.yarn|\.bun|\.git|target|dist|build"

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
done < <(find "$PROJECT_ROOT" -name package.json -not -path "*/.git/*" \
  | grep -Ev "($(echo "$SKIP_DIRS" | sed 's/|/\//g; s/^/\//; s/$/\//'))" \
  || true)

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
```

- [ ] **Step 2: Make it executable**

```bash
chmod +x action-scripts/setup-pm.sh
```

- [ ] **Step 3: Test the `find` + `grep` filtering locally**

Create a temp directory tree to verify detection:

```bash
TMP=$(mktemp -d)
mkdir -p "$TMP/app1" "$TMP/app2" "$TMP/node_modules/pkg"
echo '{"name":"app1"}' > "$TMP/app1/package.json"
touch "$TMP/app1/pnpm-lock.yaml"
echo '{"name":"app2"}' > "$TMP/app2/package.json"
touch "$TMP/app2/bun.lock"
echo '{"name":"pkg"}' > "$TMP/node_modules/pkg/package.json"

# Should find app1 and app2 but NOT node_modules/pkg
DEPUP_PATH="$TMP" bash action-scripts/setup-pm.sh
# Expected output: Detected: pnpm=1 bun=1
rm -rf "$TMP"
```

- [ ] **Step 4: Verify the `find` command correctly skips nested directories**

The `find` command with `grep -Ev` needs to filter paths containing any skip directory segment. Verify the regex handles all skip dirs:

```bash
# Test with .pnpm-store
TMP=$(mktemp -d)
mkdir -p "$TMP/.pnpm-store/v3/pkg"
echo '{"name":"store-pkg","packageManager":"pnpm@11.0.0"}' > "$TMP/.pnpm-store/v3/pkg/package.json"
DEPUP_PATH="$TMP" bash action-scripts/setup-pm.sh
# Expected: Detected: pnpm=0 bun=0
rm -rf "$TMP"
```

- [ ] **Step 5: Commit**

```bash
git add action-scripts/setup-pm.sh
git commit -m "feat(action): add automatic package manager detection and installation"
```

---

### Task 2: Wire `setup-pm.sh` into `action.yml`

**Files:**
- Modify: `action.yml:83-101` (add a step between "Configure git identity" and "Create PRs")

**Interfaces:**
- Consumes: `setup-pm.sh` from Task 1, `inputs.path` from action inputs
- Produces: PMs on PATH before `create-prs.sh` runs

- [ ] **Step 1: Add the setup step to `action.yml`**

Insert a new step after "Configure git identity" and before "Create PRs for outdated dependencies":

```yaml
    - name: Setup package managers
      shell: bash
      env:
        DEPUP_PATH: ${{ inputs.path }}
      run: |
        bash "${{ github.action_path }}/action-scripts/setup-pm.sh"
```

The full steps section in `action.yml` should read:

```yaml
  steps:
    - name: Install depup
      shell: bash
      env:
        DEPUP_VERSION: ${{ inputs.version }}
        GH_TOKEN: ${{ inputs.token }}
      run: |
        # ... existing install logic unchanged ...

    - name: Configure git identity
      shell: bash
      run: |
        git config user.name "github-actions[bot]"
        git config user.email "41898282+github-actions[bot]@users.noreply.github.com"

    - name: Setup package managers
      shell: bash
      env:
        DEPUP_PATH: ${{ inputs.path }}
      run: |
        bash "${{ github.action_path }}/action-scripts/setup-pm.sh"

    - name: Create PRs for outdated dependencies
      id: create-prs
      # ... existing step unchanged ...
```

- [ ] **Step 2: Verify `action.yml` is valid YAML**

```bash
python3 -c "import yaml; yaml.safe_load(open('action.yml'))"
```

- [ ] **Step 3: Commit**

```bash
git add action.yml
git commit -m "feat(action): wire setup-pm.sh into action workflow"
```

---

### Task 3: Update documentation

**Files:**
- Modify: `README.md:69-79`
- Modify: `docs/src/github-action/setup.md`
- Modify: `docs/src/github-action/examples.md`
- Modify: `docs/src/github-action/overview.md`

**Interfaces:**
- Consumes: completed Tasks 1-2
- Produces: updated docs reflecting that PM setup is automatic

The key documentation changes:

1. **README.md** — Remove the npm-specific setup example (pnpm/action-setup, actions/setup-node). Replace with a note that the action auto-detects and installs package managers.
2. **setup.md** — Remove the "npm Projects" section with per-PM setup examples. Replace with a short note explaining auto-detection. Keep the note about the action working for Maven, npm, and mixed projects without extra steps.
3. **examples.md** — Remove the "pnpm Project" and "Mixed Ecosystems" examples (they become identical to the minimal example). Add a note that all examples work for any ecosystem combination.
4. **overview.md** — Add a bullet to "How It Works" mentioning automatic PM detection.

- [ ] **Step 1: Update `README.md`**

Replace the npm-specific setup block (lines 69-79) with:

```markdown
The action auto-detects npm ecosystem projects (by lock file or `packageManager` field) and installs any missing package managers (pnpm, bun) before running. No extra setup steps are needed — the minimal workflow above works for Maven, npm, and mixed projects alike.
```

- [ ] **Step 2: Update `docs/src/github-action/setup.md`**

Replace the "npm Projects" section (lines 37-88) and the per-PM subsections with:

```markdown
## Package Manager Setup

The action automatically detects npm ecosystem projects in your directory tree and installs any missing package managers before running. Detection uses the same logic as the `depup` CLI: lock files (`pnpm-lock.yaml`, `package-lock.json`, `yarn.lock`, `bun.lock`) and the `packageManager` field in `package.json`.

| Package Manager | Runner Status | Action Behavior |
|-----------------|---------------|-----------------|
| npm | Pre-installed | Nothing to do |
| yarn (classic) | Pre-installed | Nothing to do |
| pnpm | Not installed | Auto-installed via corepack (if `packageManager` field exists) or npm |
| bun | Not installed | Auto-installed via npm |

No extra setup steps are needed. The minimal workflow works for Maven-only, npm-only, and mixed projects.
```

- [ ] **Step 3: Update `docs/src/github-action/examples.md`**

Remove the "pnpm Project" and "Mixed Ecosystems" sections entirely. Add a brief note to the "Minimal" example:

```markdown
> **Note:** This works for all project types — Maven, npm, pnpm, yarn, bun, or any combination. The action auto-detects and installs needed package managers.
```

- [ ] **Step 4: Update `docs/src/github-action/overview.md`**

Add to the "How It Works" section, before step 1 (Check):

```markdown
0. **Setup** — scans for npm ecosystem projects (lock files and `packageManager` fields) and installs any missing package managers (pnpm, bun)
```

- [ ] **Step 5: Build docs locally to verify**

```bash
cd docs && mdbook build && cd ..
```

- [ ] **Step 6: Commit**

```bash
git add README.md docs/
git commit -m "docs: update action docs for automatic PM detection"
```

---

### Task 4: Revert earlier doc changes (from this session)

The earlier changes in this session added `version: 11` to pnpm examples in README.md, setup.md, and examples.md. Those changes are now superseded — the action handles PM installation itself, so the examples should not show PM setup steps at all.

**Files:**
- Modify: `README.md` (revert the pnpm version addition from earlier)
- Modify: `docs/src/github-action/setup.md` (revert)
- Modify: `docs/src/github-action/examples.md` (revert)

- [ ] **Step 1: Check git diff to see what was changed earlier**

```bash
git diff README.md docs/src/github-action/setup.md docs/src/github-action/examples.md
```

- [ ] **Step 2: Apply the Task 3 changes on top — they fully replace the earlier edits**

Task 3's edits cover the same sections. When implementing Task 3, use the **original** file content as the base (before this session's changes), not the currently modified version. The simplest approach: `git checkout -- README.md docs/src/github-action/setup.md docs/src/github-action/examples.md` first, then apply Task 3.

- [ ] **Step 3: No separate commit needed — this is folded into Task 3's commit**

---

### Task 5: End-to-end verification

**Files:** None (read-only verification)

- [ ] **Step 1: Run the detection script against Elemento (local clone or temp checkout)**

```bash
TMP=$(mktemp -d)
git clone --depth 1 https://github.com/hal/elemento.git "$TMP/elemento"
DEPUP_PATH="$TMP/elemento" bash action-scripts/setup-pm.sh
# Expected: Detected: pnpm=1 bun=0, then installs pnpm
rm -rf "$TMP"
```

- [ ] **Step 2: Run against PatternFly Java**

```bash
TMP=$(mktemp -d)
git clone --depth 1 https://github.com/patternfly-java/patternfly-java.git "$TMP/pfj"
DEPUP_PATH="$TMP/pfj" bash action-scripts/setup-pm.sh
# Expected: Detected: pnpm=1 bun=0, then installs pnpm
rm -rf "$TMP"
```

- [ ] **Step 3: Run against a Maven-only project (no npm at all)**

```bash
TMP=$(mktemp -d)
mkdir -p "$TMP/maven-only"
touch "$TMP/maven-only/pom.xml"
DEPUP_PATH="$TMP/maven-only" bash action-scripts/setup-pm.sh
# Expected: Detected: pnpm=0 bun=0, no installation
rm -rf "$TMP"
```

- [ ] **Step 4: Verify action.yml is well-formed**

```bash
python3 -c "import yaml; yaml.safe_load(open('action.yml'))"
```

- [ ] **Step 5: Verify docs build**

```bash
cd docs && mdbook build && cd ..
```
