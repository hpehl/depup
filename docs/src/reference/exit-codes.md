# Exit Codes

`depup` uses granular exit codes for CI/CD integration. Each subcommand has specific codes that indicate the outcome.

## Exit Code Table

| Code | Meaning | Subcommand |
|------|---------|------------|
| 0 | All clean — no issues found | all |
| 1 | Outdated dependencies found, or update errors occurred | check, update |
| 2 | Vulnerabilities found (any severity) | audit |
| 3 | Critical or high severity vulnerabilities found | audit |

## CI Integration Examples

### Basic Pipeline Check

```bash
depup check /path/to/project
if [ $? -eq 1 ]; then
  echo "Outdated dependencies found"
fi
```

### Audit with Severity-Based Actions

```bash
depup audit --json /path/to/project
case $? in
  0) echo "Clean — no vulnerabilities" ;;
  2) echo "Vulnerabilities found (review recommended)" ;;
  3) echo "Critical/high vulnerabilities — blocking merge" ; exit 1 ;;
esac
```

### GitHub Actions Example

```yaml
- name: Check dependencies
  run: depup check --json /path/to/project
  continue-on-error: true
  id: depcheck

- name: Fail on outdated
  if: steps.depcheck.outcome == 'failure'
  run: echo "::warning::Outdated dependencies detected"
```

### Audit Gate in CI

```yaml
- name: Audit dependencies
  run: |
    depup audit --json /path/to/project
    EXIT_CODE=$?
    if [ $EXIT_CODE -eq 3 ]; then
      echo "::error::Critical or high severity vulnerabilities found"
      exit 1
    elif [ $EXIT_CODE -eq 2 ]; then
      echo "::warning::Vulnerabilities found — review recommended"
    fi
```
