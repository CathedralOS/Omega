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
- Ordinary admission uses a total internal projection from checked semantic
  state into versioned canonical evidence. Raw compiler IR is never a lock
  format. Each row may consume its earliest coherent compiler-private checked
  representation and move with it; totality belongs to the final projection,
  not one frozen source stage. This does not require a nominal Chi stage.
- Terminal evidence is separate and required only for final-realization claims
  or hardened profiles, not as a blanket package-admission gate.
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
- treating the legacy standalone local-Path compatibility scanner as package
  dependency authority.

Source fetching, content hashing, traversal limits, normalization, and graph
algorithms may be reusable only after focused review against the corrected
model.

The crate now contains reviewed building blocks for immutable Git/local
snapshots, hermetic package-name extraction, and typed package/source identity.
Mutable local-package snapshots omit only `.git` metadata and the reserved
root-level `build/` compiler output; package-authored ignore files do not control
source identity, nested `build` directories remain source, and immutable Git
materialization remains an exact selected-tree check.
Git, workspace-member, and external-local resolution now bind those pieces into
a `ResolvedPackageSource`: declaration and identity come from the immutable
snapshot and canonical source lineage, and canonical literal dependency rows
are projected without executing build code. Workspace-member resolution binds
the workspace root lineage to a normalized member-relative path, verifies the
live member is the matching strict canonical descendant, and snapshots only
that member. A transport-neutral recursive resolver accepts only erased custody
derived from these resolved sources, delegates each request to an adapter, and
returns the complete validated `ResolvedPackageClosure` together with every
exact immutable custody root. It derives ordinary aliases from fetched package
declarations, preserves explicit aliases, reuses identical custody, and reports
all requesting paths when one package key resolves inconsistently. Package,
dependency-request, and depth ceilings bound hostile closure traversal.
The first concrete adapter roots traversal in an explicitly supplied workspace
member, resolves requester-relative Path rows only within a registered
workspace, and resolves Git rows through immutable Git custody. A fetched Git
snapshot becomes a separate registered workspace for its own nested Path rows;
the adapter never searches parents or guesses an external protocol.
Toolchain/compiler evidence is intentionally absent, and this closure has no
persistence, lock, build-execution, or admission API. `PackageKey` also
derives the opaque stable identity carrier used by package-aware compiler
inputs. The compiler's separate native and checked package entrypoints consume
a closed requester-local alias graph and canonical source roots without
consulting downloaded dependency rows; legacy standalone compilation still
retains a narrow explicit `depend_as(..., Source::Path { ... })` compatibility
scanner. A package-side handoff translates only the validated custody closure
into compiler inputs, whose constructor independently canonicalizes and checks
every root and edge again. Checked package compilation now also retains
the exact root package and selected build-machine symbol and can emit an
in-memory authority review projection for one explicit target. That projection
is intentionally not source/toolchain-bound admission evidence: exact toolchain
ownership gaps remain explicit. Compiler-generated symbols now inherit the
package/toolchain provenance of a mandatory authored derivation origin, while
truly source-free symbols remain unresolved. The projection includes selected provider
mechanisms, and provider plans/trust rows retain exact package owners for the
realizing machine, provider type, service schema, and requirement owner.
Checked-adapter bindings resolve by canonical overload plus exact package owner
without a short-name fallback. Authored provider choices retain two structural
type paths, resolve to exact typed trait/data symbols, and match plans only by
package plus canonical path. The selected plans remain intact through cycle,
ABI, and checked-fact construction; package-distinct same-spelled slots and
providers do not collapse. Remaining schema/grant joins, compiler-intrinsic
toolchain identity, and the
remaining trust/proof/reproducibility joins are incomplete. Build-bound
progress obligations retain and match package ownership for both service and
requirement, and retained selected-provider facts expose no name-only plan
lookup. Installation-bound reach, termination, mutation, crash, and permission
frontier rows now use normalized package-owned semantic paths, and crash
predicates retain their existing source-independent canonical identity. Review
identity retains the exact deployment profile rather than collapsing profiles
that happen to share a native ABI. Capability-flow states, including propagated
`via` states, are package-qualified. Ordinary public-machine visibility now
survives checked compilation; public omission enforces empty reach, invocation,
suspension, blocking, and crash ceilings. The review includes public and
boundary callables plus the selected build machine, excludes private machines,
and projects invocation targets as exact parameter ordinals or package-qualified
service identities. Package-qualified type identity gives every non-binder
nominal an exact package, toolchain, or unresolved owner while preserving
owner-free alpha-normalized binders. Package-owned public data is now projected
with supply, generic shape, properties, stable field/variant identities,
retired identities, relevance, and exact field/payload types. Numbered ordinary
data is the wire contract; the retired standalone `wire data` form is not a
second API row. Quotients, data `where` facts, and static machine/proposition
parameters reject review until exact canonical rows exist. Public traits now
retain exact package identity, boundary status, alpha-normalized type/const
binders, package-qualified parent applications, and ordered machine/operator
requirement signatures. Trait/requirement lifetimes, conformance bounds,
invariants, default realizations, and operational/proof/crash/termination
contracts fail closed until complete rows land. Public domains with
representable shape now retain exact package identity, alpha-normalized generic
carrier/index shape, package-qualified carrier/index types, closed compiler-owned
classifications, and authorized establishment routes with exact
package-qualified trait/requirement identities. Transparent aliases recursively
flatten to canonical package-qualified atoms; compiler carry atoms remain
explicitly toolchain-unbound. Predicates, semantic roles, and operators fail
closed until canonical rows land. The compiler now provides a version-7
length-framed binary comparison encoding over this review
projection; it is explicitly not a package certificate or accepted-lock
payload. Raw Rust/debug serialization is not an alternative. These pieces do
not become an admission path until the legacy name-keyed lock APIs are replaced
and sealed, locally regenerated compiler evidence plus the hardened resolver
receipt are wired through end to end. The earlier public
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
|   |-- dependency_edit.rs # Digest-bound conservative build.omg edit plans.
|   |-- graph.rs           # Typed pre-admission source reconciliation.
|   |-- closure_resolution.rs # Bounded recursive immutable source custody.
|   |-- source_adapter.rs  # Explicit workspace and Git closure policy.
|   |-- compiler_handoff.rs # Revalidated package-aware compiler inputs.
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
