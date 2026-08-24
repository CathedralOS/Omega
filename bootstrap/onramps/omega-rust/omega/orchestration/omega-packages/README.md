# Omega Packages

This is the Rust on-ramp home for package source resolution, graph
reconciliation, admission, audit, and install/update orchestration.

The package manager is Cargo-like in workflow, not in registry model. It
resolves Git, URL, and local sources supplied by the project; it does not host
packages or trust repository names.

The security and custody rules are durable; the exact build-library vocabulary
is intentionally discovery-driven. Prefer existing Omega data, machines,
arithmetic, and provider mechanisms, and add a new public boundary only when a
real package fixture demonstrates an irreducible external contract.

## Governing model

- Every fetched package declares its own human name through the hermetically
  evaluated `PACKAGE` constant in its `build.omg`.
- `PackageName` is presentation. `PackageKey` joins the name to canonical
  source lineage. `PackageInstance` adds exact source, toolchain, and checked
  package-evidence identity.
- `build.omg` records source requests and update selectors. The dependency's
  own declaration determines its default import alias; `--as` is an
  exceptional local rename.
- Dependency-source projection is effect-free and completes before downloaded
  build code receives any host provider.
- The compiler, not the package or CLI caller, derives package capability/API
  evidence from checked source and build results.
- `omega.lock` records the exact reconciled closure and normalized accepted
  evidence baseline. It should normally be committed.
- Every update receives source/provenance triage. Evidence changes block on a
  conflict; retained dangerous authority always recommends code audit.
- Missing old source escalates code review but does not prevent comparison
  against the lock baseline. Missing lock evidence causes fresh graph
  admission.

The complete design is in:

- `wiki/design_briefs/package_manager_first_draft.md`
- `wiki/design_briefs/build_and_package_model.md`
- `wiki/language_guide/chapter_15_modules_imports_visibility.md`
- `wiki/language_guide/chapter_19_capabilities_effects_boundaries.md`
- `SOURCE_RESOLVER_SECURITY.md` for the resolver helper, snapshot, sandbox, and
  receipt boundary.

## Trust status

The current crate is exploratory scaffolding, not an accepted package-admission
implementation. No current manifest-file, receipt-file, lock-assembly, or plan
CLI is a production trust boundary.

The following assumptions are superseded and must be removed before
`omega install` or `omega update` can mutate project state:

- locks keyed by package-authored name alone;
- mandatory caller-supplied alias and package name;
- caller-constructed `PackageCapabilityManifest` values;
- standalone JSON manifests accepted as compiler evidence;
- a manifest fingerprint without the normalized accepted baseline;
- free-form reviewer/reason receipts that accept whole sections; and
- syntactic dependency scanning that silently ignores malformed package builds.

Source fetching, content hashing, traversal limits, normalization, and graph
algorithms may be reusable only after focused review against the corrected
model.

The crate now contains reviewed building blocks for immutable Git/local
snapshots, hermetic package-name extraction, and typed package/source/instance
identity. They do not become an admission path until the legacy name-keyed
graph and lock APIs are replaced and compiler-issued evidence plus the hardened
resolver receipt are wired through end to end.

## Target command surface

```text
omega install <source> [--rev <revision>] [--as <alias>]
omega update [package-or-alias...] [--to <revision>]
omega audit packages
```

Install fetches the source before learning its package name. Update builds from
the accepted lock and never silently re-resolves mutable source selectors.
Conflict resolution is row-specific and bound to the exact candidate; there is
no blanket approval switch.

The former commands accepting `manifest.json`, `receipt.json`, `--package`, or
mandatory `--alias` are quarantined from the production CLI. Their internal
library scaffolding remains only for isolated tests while the typed replacements
are built; invoking the old command names fails before parsing or writing any
artifact.

## Responsibilities

- Normalize transport-independent source lineage where equivalence can be
  established safely.
- Resolve source requests to immutable commit/tree/content identity in an
  isolated cache.
- Extract package declaration and dependency-source projection without
  build-host authority.
- Reconcile one immutable instance per `PackageKey` in the initial model.
- Invoke compiler package-admission mode and accept only compiler-issued
  evidence bound to source and toolchain identity.
- Persist the complete accepted baseline and exact closure in `omega.lock`.
- Render compact capability conflicts and hostile-input-safe LLM triage packets.
- Perform conservative `build.omg` edits only after admission.

## Non-responsibilities

- Hosting packages or providing a registry namespace.
- Trusting a package name, URL spelling, repository name, or human prose as
  identity/evidence.
- Solving semantic-version ranges in the first implementation.
- Defining language semantics for reach, authority, proofs, providers, or build
  observations.
- Giving downloaded code resolver, root-package, or acceptance authority.

## Expected structure

```text
omega-packages/
|-- README.md
|-- src/
|   |-- identity.rs        # PackageName, PackageKey, PackageInstance.
|   |-- source.rs          # Source requests, lineage, immutable resolution.
|   |-- resolver.rs        # Fetch/cache boundary and transport receipts.
|   |-- declaration.rs     # Hermetic PACKAGE extraction.
|   |-- graph.rs           # Dependency projection and reconciliation.
|   |-- evidence.rs        # Compiler-issued package admission evidence.
|   |-- lock.rs            # Accepted closure and evidence baseline.
|   |-- conflict.rs        # Row-specific admission conflicts/resolutions.
|   |-- audit.rs           # Source/provenance/capability audit rendering.
|   |-- install.rs         # Fetch, derive, admit, then edit/write.
|   |-- update.rs          # Candidate reconciliation and admission.
|   `-- commands.rs        # Thin CLI-facing orchestration.
`-- tests/
    |-- identity.rs
    |-- install.rs
    |-- update.rs
    |-- audit.rs
    `-- remote_fixtures.rs
```

Machine persistence format is an internal encoding choice. Human review and
conflict surfaces use concise canonical text and do not expose package-authored
prose to the triage model.

## Fixtures

The local fixture corpus is under `fixtures/packages/`; exact remote Git pins
are recorded in `fixtures/packages/REMOTE_PINS.md`. Before these are admission
fixtures rather than source-resolution fixtures, each must declare `PACKAGE`
and have its evidence emitted by the compiler. Tests that fabricate manifests
from fixture intent do not establish package security behavior.
