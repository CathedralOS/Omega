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
- Reject suspect package admission during install/update unless an exact
  capability-conflict resolution artifact is supplied; surface recommended
  audit findings for retained intrinsically dangerous authority.
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

## Open Design Gates

The current crate deliberately stops before mutating `build.omg` or deriving
package manifests from compiler output. The remaining gates are owner-level
language/compiler decisions, not package-crate policy choices:

- the exact authored `build.omg` dependency API and conservative edit contract;
- the final storage and verification surface for capability-change review
  receipts;
- the install-time sequence for static preflight versus dependency
  `build.omg` execution; and
- the checked evidence boundary for package capability manifests. The existing
  executable capability manifest is entry-oriented and must not be treated as
  package admission evidence for libraries.

## Expected Structure

The first crate boundary can stay deferred until APIs settle. When it becomes a
workspace crate, use this directory as the crate root.

```text
omega-packages/
|-- README.md
|-- src/
|   |-- lib.rs
|   |-- audit.rs           # Resolved graph audit reporting.
|   |-- source.rs          # Source specs, URL/path identity, immutable pins.
|   |-- resolver.rs        # Fetch/cache boundary and transport receipts.
|   |-- manifest.rs        # Package capability manifest model.
|   |-- install.rs         # Non-mutating install plan/admission preview.
|   |-- json.rs            # Strict internal JSON parser for machine artifacts.
|   |-- lock.rs            # Full package-closure lock artifact.
|   |-- diff.rs            # Capability-manifest comparison and severity.
|   |-- update.rs          # Default update admission decisions.
|   |-- build_omg.rs       # Guided `build.omg` dependency edits.
|   `-- commands.rs        # install/update/audit orchestration entrypoints.
`-- tests/
    |-- local_fixture_graph.rs
    |-- install.rs
    |-- update.rs
    |-- audit.rs
    `-- remote_fixtures.rs
```

The CLI should remain a thin adapter. It should parse command arguments and
delegate behavior here.

## Fixtures

Local package-manager fixtures live under `fixtures/packages/` at the repository
root. They use normal kebab-case package names and are resolved as local source
directories first. Private GitHub mirrors under `CathedralOS` are recorded with
exact initial commit pins in `fixtures/packages/REMOTE_PINS.md`; remote tests
should use those pins rather than branch names. The optional private-network
smoke test is:

```text
cargo test -p omega-packages --test remote_fixtures -- --ignored --test-threads=1
```

## Current Slices

- `commands`: internal source-audit command API plus CLI-ready source locator
  parsing for local paths, `file://`, HTTPS Git URLs, and SSH/scp-style Git
  locators. It also contains locator-backed source audit, locator-backed
  source-cache policy records and record writes, capability-change receipt creation,
  manifest-file backed lock assembly, lock-file backed install/update plan
  commands, and lock-file plus manifest-file backed graph-audit command seams
  for future CLI wiring.
- `audit`: resolved package-graph audit over locks and manifests, including
  dependency paths for exported service reach and fail-closed consistency
  checks. Audit rows surface source identity, dependency aliases, provider
  requirements/selections, trust receipts, and capability-flow verb counts.
- `manifest`: canonical package/alias names, normalized package capability
  manifests, strict JSON parsing, standalone atomic read/write, SHA-256
  fingerprints, and manifest diffs.
- `diff`: severity-ranked manifest deltas with concrete reviewer guidance for
  public service, build-host service, provider-requirement, and capability-flow
  changes.
- `install`: non-mutating install plan assembly for future `omega install`
  wiring. It validates the current graph, verifies the root candidate manifest
  binds the requested alias to the requested package, assembles and audits the
  candidate lock, and reports newly added package identities. Final install
  wiring should route suspect initial authority through the same capability
  conflict/resolution admission path as update. The command seam can read the
  current lock file before returning the plan.
- `lock`: machine-written package closure records with resolved source
  identity, manifest fingerprints, dependency aliases, trust receipts, stable
  JSON, lock fingerprints, closed-and-reachable closure validation, and strict
  lock-file persistence. Locks can be assembled from compiler-supplied package
  capability manifests before writing.
- `review`: deterministic capability-change review receipts bound to exact
  source identities, manifest fingerprints, accepted diff sections, reviewer,
  and reason. Receipts have strict JSON parsing plus standalone atomic
  read/write support for the current command seam. The settled UX is to replace
  standalone approval prompts with Omega-generated capability conflicts, exact
  resolution artifacts, and lock references to admitted resolution evidence.
  Command-level creation rejects no-op, source-only, and package-mismatched
  updates.
- `resolver`: source-cache policy records for local/Git resolution, including
  limits, path/submodule policy, resolved identities, success/rejection verdict,
  stable JSON, strict parsing, atomic persistence, and record fingerprints.
- `source`: local-path source identity with deterministic hashing, `.git`
  directory exclusion, traversal limits, symlink escape rejection, and Git
  clone/fetch resolution to exact commit/tree identity.
- `update`: package-update admission that permits source-only changes, rejects
  non-source capability manifest deltas with review guidance, and admits
  capability-changing updates only with exact matching review receipts in the
  current command seam. It can also produce a non-mutating lock update plan that
  assembles a candidate lock only after policy admission and candidate graph
  audit. The command seam can read the current lock file plus an optional
  standalone receipt file before returning the plan; final update wiring should
  emit capability conflicts and recommended audit findings for retained
  filesystem, network, process, dynamic-loader, signing, secret, or equivalent
  authority.

## Current CLI Surface

The Rust on-ramp `omega` binary exposes package/source audit and evidence
paths:

```text
omega audit source <locator> [--rev <rev>] [--cache-dir <dir>]
omega audit source-cache-policy <locator> [--rev <rev>] [--cache-dir <dir>] [--out <record.json>]
omega audit packages [--lock <omega.lock>] --manifest <manifest.json>...
omega review capability-change --old-manifest <manifest.json> --new-manifest <manifest.json> --reviewer <id> --reason <text> --out <receipt.json>
omega plan install --lock <omega.lock> --current-manifest <manifest.json>... --candidate-manifest <manifest.json>... --alias <alias> --package <package>
omega plan update --lock <omega.lock> --current-manifest <manifest.json>... --candidate-manifest <manifest.json>... --package <package> [--receipt <receipt.json>]
omega lock assemble --root-package <package> --manifest <manifest.json>... --out <omega.lock>
```

The source audit resolves a local/Git locator to content identity and reports
the resolved commit/tree when applicable. The source-cache-policy audit prints
the deterministic cache policy record, including rejected verdicts, and writes
it when `--out` is supplied. The package graph audit requires precomputed
package capability manifest files. The review command writes an explicit
standalone capability-change receipt to the requested path. The plan commands
read an existing lock plus explicit current/candidate manifest files and print
install/update admission plans. The lock assembly command writes only the
explicit `--out` lock file from explicit manifest files after closure
validation and graph audit. These commands do not derive package manifests,
execute dependency `build.omg`, or edit `build.omg`.
