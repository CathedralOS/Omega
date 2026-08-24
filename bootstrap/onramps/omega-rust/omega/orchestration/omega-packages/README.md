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
- Every update receives source/provenance triage. Blocking capability/API
  changes produce exact conflicts; retained dangerous authority always
  recommends code audit.
- Claim-free opaque boundary data remains visible as package-qualified
  representation-TCB evidence. Introduction or material change recommends
  code/ABI audit without becoming a trust claim unless exact mechanism,
  authority, executable, claim, or compatibility policy independently blocks.
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
snapshots, hermetic package-name extraction, and typed package/source identity.
Git and external-local resolution now bind those pieces into a
`ResolvedPackageSource`: declaration and identity come from the immutable
snapshot and canonical source lineage, and canonical literal dependency rows
are projected without executing build code. Toolchain/compiler evidence is
intentionally absent. A separate `ResolvedPackageClosure` validates exact typed
source topology but has no persistence or admission API. `PackageKey` also
derives the opaque stable identity carrier used by package-aware compiler
inputs. The compiler's separate native and checked package entrypoints consume
a closed requester-local alias graph and canonical source roots without
consulting downloaded dependency rows; legacy standalone compilation still
retains its transitional scanner. Checked package compilation now also retains
the exact root package and selected build-machine symbol and can emit an
in-memory authority review projection for one explicit target. That projection
is intentionally not source/toolchain-bound admission evidence: exact toolchain
ownership gaps remain explicit. Compiler-generated symbols now inherit the
package/toolchain provenance of a mandatory authored derivation origin, while
truly source-free symbols remain unresolved. The projection includes selected provider
mechanisms, and provider plans/trust rows retain exact package owners for the
realizing machine, provider type, service schema, and requirement owner.
Checked-adapter bindings resolve by canonical overload plus exact package owner
without a short-name fallback. Authored provider choices now resolve once to
exact plans which remain intact through selected-cycle, ABI, and checked-fact
construction; package-distinct same-spelled selected slots do not collapse,
and ambiguous readable invocation targets reject. Authored selector resolution,
remaining schema/grant joins, compiler-intrinsic toolchain identity, and the
remaining trust/proof/reproducibility joins are incomplete. Build-bound
progress obligations retain and match package ownership for both service and
requirement, and retained selected-provider facts expose no name-only plan
lookup. Installation-bound reach, termination, mutation, crash, and permission
frontier rows now use normalized package-owned semantic paths, and crash
predicates retain their existing source-independent canonical identity. Review
identity retains the exact deployment profile rather than collapsing profiles
that happen to share a native ABI. Capability-flow states, including propagated
`via` states, are package-qualified. The compiler now provides a versioned,
length-framed binary comparison encoding over this review projection; it is
explicitly not a package certificate or accepted-lock payload. Raw Rust/debug
serialization is not an alternative. These pieces do not become an
admission path until the legacy name-keyed lock APIs are replaced and sealed,
locally regenerated compiler evidence plus the hardened resolver receipt are
wired through end to end. The earlier public
`PackageInstance` constructor was removed: the real type must not exist as a
caller-constructible tuple of arbitrary toolchain and evidence fingerprints.

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
mandatory `--alias` are quarantined from the production CLI. Their manifest,
lock, review, install, update, and audit modules compile only for isolated crate
tests while the typed replacements are built; they are absent from the release
library API. Invoking the old command names fails before parsing or writing any
artifact.

## Responsibilities

- Normalize transport-independent source lineage where equivalence can be
  established safely.
- Resolve source requests to immutable commit/tree/content identity in an
  isolated cache.
- Extract package declaration and dependency-source projection without
  build-host authority.
- Reconcile one immutable instance per `PackageKey` in the initial model.
- Invoke compiler package-admission mode and locally regenerate evidence bound
  to source, evidence schema, and compiler/toolchain provenance. This excludes
  dependency-authored manifests; it does not certify the selected compiler.
- Persist the complete accepted baseline and exact closure in `omega.lock`.
- Render compact capability conflicts and hostile-input-safe LLM triage packets.
- Leave audit quality, reviewer/quorum requirements, and merge authorization to
  root-project policy; no receipt or status is presented as proof of audit.
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
|   |-- identity.rs        # Package/source lineage and instance identity.
|   |-- source.rs          # Source requests and immutable snapshots.
|   |-- package_source.rs  # Snapshot-to-declared-PackageKey custody.
|   |-- resolver.rs        # Fetch/cache boundary and transport receipts.
|   |-- declaration.rs     # Hermetic PACKAGE extraction.
|   |-- dependency_projection.rs # Hermetic literal source requests.
|   |-- graph.rs           # Typed pre-admission source reconciliation.
|   |-- source_commands.rs # Unhardened source diagnostic command surface.
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

The checked package-review path also fails closed on contract-entailment
stand-downs. It audits the pristine typed graph (including generic templates),
retains compiler-owned machine/contract/fact coordinates and a closed reason,
and refuses review when any checked-implementation claim was left unjudged. Accepted or
opaque supply remains trust-bearing. These rows are currently in-memory review
state, not sealed lock evidence.

## Fixtures

The local fixture corpus is under `fixtures/packages/`; exact remote Git pins
are recorded in `fixtures/packages/REMOTE_PINS.md`. Before these are admission
fixtures rather than source-resolution fixtures, each must declare `PACKAGE`
and have its evidence emitted by the compiler. Tests that fabricate manifests
from fixture intent have been removed from integration coverage.
