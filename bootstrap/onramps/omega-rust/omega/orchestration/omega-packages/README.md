# Omega Packages

This is the planned home for the Rust on-ramp's package-resolution and package
audit orchestration.

The package manager is Cargo-like in workflow, not in registry model. It does
not host packages or perform version solving. It resolves user-provided sources
such as Git URLs, HTTPS/SSH remotes, and local paths to exact content identity,
then gates the resolved package through Omega's existing build, reach,
capability, provider, and trust evidence.

## Naming

Package identity names, repository names, and lock package keys use kebab-case,
for example `generated-table`. Omega aliases and Rust identifiers use
snake_case where an identifier is required, for example `generated_table`.
The resolver should treat package identity spelling as canonical so
`foo-bar` and `foo_bar` cannot silently become different packages in one
package namespace.

## Responsibilities

- Resolve source locations to immutable package content.
- Maintain the package source cache with path-containment and expansion limits.
- Read and update dependency bindings in `build.omg`.
- Extend the machine-written lock artifact with resolved package closure
  evidence.
- Produce package capability manifests from compiler-derived evidence.
- Reject updates when a package capability manifest changes unless an explicit
  review/acceptance receipt is supplied.
- Provide the orchestration API used by `omega install`, `omega update`, and
  `omega audit packages`.

## Non-Responsibilities

- Hosting packages.
- Solving semver ranges.
- Defining language semantics for `reaches`, authority, domains, proofs, or
  build observations.
- Granting provider or root authority on behalf of dependency packages.
- Letting downloaded `build.omg` code inherit resolver or root-package
  authority.

Psi remains responsible for source parsing and target-neutral semantic
evidence. Omega orchestration is responsible for resolver authority,
admission, audit presentation, lock records, and CLI workflow.

## Expected Structure

The first crate boundary can stay deferred until APIs settle. When it becomes a
workspace crate, use this directory as the crate root.

```text
omega-packages/
|-- README.md
|-- src/
|   |-- lib.rs
|   |-- source.rs          # Source specs, URL/path identity, immutable pins.
|   |-- resolver.rs        # Fetch/cache boundary and transport receipts.
|   |-- manifest.rs        # Package capability manifest model.
|   |-- lock.rs            # Full package-closure lock artifact.
|   |-- diff.rs            # Capability-manifest comparison and severity.
|   |-- build_omg.rs       # Guided `build.omg` dependency edits.
|   `-- commands.rs        # install/update/audit orchestration entrypoints.
`-- tests/
    |-- install.rs
    |-- update.rs
    `-- audit.rs
```

The CLI should remain a thin adapter. It should parse command arguments and
delegate behavior here.

## Current Slices

- `manifest`: canonical package/alias names, normalized package capability
  manifests, stable JSON, SHA-256 fingerprints, and manifest diffs.
- `lock`: machine-written package closure records with resolved source
  identity, manifest fingerprints, dependency aliases, trust receipts, stable
  JSON, and lock fingerprints.
- `review`: deterministic capability-change review receipts bound to exact
  source identities, manifest fingerprints, accepted diff sections, reviewer,
  and reason.
- `source`: local-path source identity with deterministic hashing, `.git`
  directory exclusion, traversal limits, symlink escape rejection, and Git
  clone/fetch resolution to exact commit/tree identity.
