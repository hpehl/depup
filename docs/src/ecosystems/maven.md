# Maven

`depup` scans multi-module Maven projects and checks dependency versions against upstream Maven repositories.

## What Gets Discovered

### Property References

Any `${...}` property used as a version in a `<dependency>` or `<plugin>` block is discovered. This includes properties with any naming convention:

- `${junit.version}`
- `${version.wildfly}`
- `${my.lib.version}`
- `${jackson-databind.version}`

The only exclusion is `${project.*}` properties, which are Maven built-ins.

Property values are resolved from the root POM's `<properties>` block. Chained references (a property referencing another property) are supported up to 10 levels deep.

### Plain Inline Versions

Artifacts with hardcoded version numbers are also checked:

```xml
<dependency>
    <groupId>com.example</groupId>
    <artifactId>my-lib</artifactId>
    <version>5.10.0</version>
</dependency>
```

### Tool Versions

Version properties in Maven POMs that reference tool versions are discovered and checked against their respective registries:

| Property Pattern | Tool | Registry |
|-----------------|------|----------|
| `version.node` | Node.js | Node.js distribution index |
| `version.npm` | npm | npm registry |
| `version.pnpm` | pnpm | npm registry |
| `version.yarn` | Yarn | npm registry |

## Multi-Module Projects

`depup` parses the root `pom.xml` and recursively follows `<modules>` declarations to discover all modules. Properties defined in the root POM are available to all child modules.

This is a key advantage over Maven's built-in `versions:display-property-updates`, which fails when properties are defined in a parent POM but referenced in child POMs.

## Custom Repositories

If an artifact is not found on Maven Central, `depup` queries all `<repositories>` and `<pluginRepositories>` defined across the project's POMs. Repository URLs are collected from all POMs and deduplicated.

- `<repositories>` entries are queried for dependencies
- `<pluginRepositories>` entries are queried for plugins

Queries to custom repositories run in parallel for performance.

## Version Resolution

`depup` resolves versions by fetching `maven-metadata.xml` from Maven repositories. It tries Maven Central first, then falls back to custom repositories.

Version comparison uses Maven-aware ordering that correctly handles qualifiers like `.Final`, `-SP1`, `-RELEASE`, and other Maven conventions that don't follow strict semver.

## Updating Maven Dependencies

When running `depup update`, version numbers in POM files are rewritten in place while preserving all formatting, comments, and indentation:

- **Managed properties** — the value in the `<properties>` block is updated
- **Plain inline versions** — the `<version>` element content is updated

The updater is surgical: it only changes the version text, leaving the rest of the XML structure untouched.

## Requirements

- Network access to Maven Central (`repo1.maven.org`)
- Network access to any custom repositories defined in the project's POMs
- Maven Central requires a `User-Agent` header; `depup` sets `depup/{version}`

## Known Quirks

- Artifacts not on Maven Central that also aren't in any POM-defined repository will show as errors
- `${project.*}` properties are always skipped (they're Maven built-ins like `${project.version}`)
