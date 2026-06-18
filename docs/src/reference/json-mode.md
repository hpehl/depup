# JSON Mode

Use `--json` for machine-readable output on any subcommand:

```bash
depup check --json
depup update --json
depup audit --json
```

When `--json` is active:

- Progress bars are suppressed
- Output is valid JSON printed to stdout
- Errors produce structured JSON instead of human-readable messages

## Output Format

### Check

The check output is a JSON array of dependency objects:

```json
[
  {
    "ecosystem": "maven",
    "kind": "dependency",
    "artifact": "org.junit.jupiter:junit-jupiter",
    "current": "5.10.0",
    "latest": "5.11.3",
    "status": "outdated"
  }
]
```

### Update

The update output includes the update status:

```json
[
  {
    "ecosystem": "maven",
    "kind": "dependency",
    "artifact": "org.junit.jupiter:junit-jupiter",
    "current": "5.10.0",
    "latest": "5.11.3",
    "status": "updated"
  }
]
```

### Audit

The audit output includes vulnerability details:

```json
[
  {
    "ecosystem": "maven",
    "kind": "dependency",
    "artifact": "com.example:vulnerable-lib",
    "version": "1.2.3",
    "vulnerability": {
      "id": "GHSA-xxxx-xxxx-xxxx",
      "summary": "Remote code execution vulnerability",
      "severity": "critical"
    }
  }
]
```

## Error Envelope

When an error occurs in JSON mode, a structured error object is returned instead of an array:

```json
{
  "error": {
    "code": "POM_NOT_FOUND",
    "message": "No pom.xml found in /nonexistent"
  }
}
```

### Error Codes

| Code | Description |
|------|-------------|
| `POM_NOT_FOUND` | No `pom.xml` found at the specified path |
| `POM_PARSE_FAILED` | Failed to parse the POM XML |
| `HTTP_REQUEST_FAILED` | Network request failed (e.g., to Maven Central) |
| `CLAP_PARSE_ERROR` | Invalid command-line arguments |
| `INTERNAL` | Unexpected internal error |
