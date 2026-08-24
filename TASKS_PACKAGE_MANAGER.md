# Tasks: Package Manager

Status: corrected implementation plan, 2026-08-23.

This file tracks the Cargo-like source/package service under `omega`. The
governing design is:

- `wiki/design_briefs/package_manager_first_draft.md`
- `wiki/design_briefs/build_and_package_model.md`
- `wiki/language_guide/chapter_15_modules_imports_visibility.md`
- `wiki/language_guide/chapter_19_capabilities_effects_boundaries.md`

## Trust status

All current `omega-packages` code predates the corrected identity and admission
model. Treat it as suspect scaffolding. Nothing that accepts caller-authored
manifest JSON, package names, aliases, or free-form review receipts may become a
production trust input. Existing source/hash/graph code is reusable only after
focused review.

Do not wire mutating `omega install` or `omega update` until every P0 task is
complete.

## Settled model

- A fetched package declares `const PACKAGE: Package` in its own `build.omg`.
- `PackageName` is human identity; `PackageKey` adds canonical source lineage;
  `PackageInstance` adds exact source, toolchain, and compiler evidence.
- Normal dependency rows name only a source request. Default aliases are
  derived from the fetched package name; explicit aliases are exceptional.
- Dependency-source projection is hermetic and completes before dependency
  build execution.
- `build.omg` records update intent. `omega.lock` records exact reconciliation
  and the normalized accepted capability/API baseline.
- Capabilities are compiler-derived from checked candidate source/build output.
- Install compares against an empty baseline. Missing lock evidence causes
  fresh graph admission. Missing old source causes standalone source audit but
  does not erase a valid lock baseline.
- Every update receives source/provenance triage. Capability/API changes block;
  retained dangerous authority always recommends code audit.
- Conflict resolution is row-specific and candidate-bound, never a blanket
  yes/no receipt.
- Implementation vocabulary is discovery-driven: reuse ordinary Omega data,
  machines, arithmetic, and existing provider machinery; do not add a public
  boundary trait or package-specific policy axis unless a concrete fixture
  exposes an irreducible contract that needs it.

## P0 — Replace invalid foundations

- [x] **PACKAGE-SCAFFOLDING-AUDIT.** Review every production path in
  `omega-packages` and classify it as retain, rewrite, or delete.

  Known suspect evidence points include unrestricted string identities,
  section fingerprints produced from Rust `Debug` output, coarse section-level
  rather than checked-row diffs, capability-flow counts without provenance,
  and lock trust-receipt identifiers without retained evidence.

  Acceptance: the review covers source/cache process isolation, identity,
  manifests, locks, install/update plans, graph audit, review receipts, CLI
  exposure, persistence, and test provenance. No retained API accepts an
  unverified caller-constructed security artifact.

  Completed 2026-08-23: the file-by-file and trust-path classification is in
  [`omega-packages/SCAFFOLDING_AUDIT.md`](bootstrap/onramps/omega-rust/omega/orchestration/omega-packages/SCAFFOLDING_AUDIT.md).

- [x] **QUARANTINE-PROTOTYPE-CLI.** Prevent the existing manifest-file,
  receipt-file, lock-assembly, and install/update-plan commands from being
  mistaken for package admission.

  Acceptance: help/output labels them diagnostic and untrusted, or the commands
  are removed until compiler-issued evidence exists. No command can write an
  accepted production lock from standalone JSON manifests.

  Completed 2026-08-23: manifest-based package audit, plan, review, and lock
  commands are absent from production help and reject before parsing or writing
  artifacts. Source-audit commands remain separately marked as unhardened until
  `HARDEN-SOURCE-RESOLVER` closes.

- **HARDEN-SOURCE-RESOLVER.** Re-audit the current Git/local resolver as a
  hostile-input boundary.

  The production helper/snapshot/receipt contract is recorded in
  [`SOURCE_RESOLVER_SECURITY.md`](bootstrap/onramps/omega-rust/omega/orchestration/omega-packages/SOURCE_RESOLVER_SECURITY.md).

  Progress 2026-08-23: diagnostic source commands now require an explicit
  `local` or `git` adapter; unknown URLs are no longer guessed to be Git. Local
  source identity now uses injective versioned framing over raw relative-path
  bytes, entry kind, directory presence, symlink target spelling, Unix
  executable mode, length, and content, rejects special files and links into
  excluded Git metadata, and checks entry limits before allocation. Directory
  permissions normalize to the read-only snapshot policy rather than preserving
  irrelevant host-checkout state. Git caches now use full policy-versioned
  keys, exclusive per-entry locking, staged publication, exact resolver
  metadata/origin/config verification, sealed Git configuration, and
  pre-materialization rejection of `.gitmodules` and gitlinks. Git source is
  now read from validated tree/blob objects, materialized without checkout,
  filters, hooks, or submodules, re-hashed against the expected tree, made
  read-only, and atomically published as a resolver-owned snapshot. Published
  snapshots are revalidated before reuse. Local sources now follow the same
  custody shape: a bounded capture is re-materialized into a content-addressed,
  read-only, atomically published resolver snapshot; source/cache overlap and
  ordinary concurrent mutation reject, and diagnostics expose the snapshot
  path rather than the live tree.

  Remaining suspect points:

  - local capture still includes tool-owned build outputs already present in
    the source root, and its before/after check does not defend against a
    deliberately hostile same-user process racing both observations;
  - cache locking coordinates resolver processes but is not protection against
    an independently hostile process that can mutate the cache directory;
  - the Git subprocess has no OS sandbox or resource ceilings, and SSH transport
    still necessarily invokes an external client with its own configuration
    surface;
  - Git tree-list and general command output are still captured without a
    strict process-memory ceiling;
  - resolver process/network/filesystem authority is not yet represented by a
    hardened execution boundary and receipt.

  Acceptance: cache ownership/origin is verified, identities use full
  collision-resistant keys, Git runs with sealed configuration in an isolated
  process boundary, materialization/archive policy is enforced before
  consumption, and source hashing is injective over every filesystem
  distinction that can affect compilation or build execution.

## P1 — Package declaration and identity

- **PACKAGE-DECLARATION-VOCABULARY.** Add the toolchain-owned `Package` build
  data and require exactly one `const PACKAGE: Package` in each package
  `build.omg`.

  Acceptance: extraction occurs hermetically before dependency resolution or
  build execution and rejects missing, duplicate, effectful, generated,
  dependency-dependent, or invalidly spelled declarations.

  Progress 2026-08-23: the compiler prelude owns `Package { name: &[u8] }`, and
  `omega-packages` now extracts the exact literal declaration through the
  ordinary Psi lexer/parser without loading imports or executing code. It
  rejects package-authored `Package`, malformed/scoped/duplicate declarations,
  nonliteral initializers, invalid bytes, and names that cannot map to a default
  Omega alias. Git and external-local source custody now extracts this
  declaration from the resolver-owned immutable snapshot and joins it to typed
  lineage; dependency projection and complete closure resolution remain.

- **PACKAGE-KEY-AND-INSTANCE.** Replace name-keyed graph and lock APIs with
  `PackageKey` and `PackageInstance`.

  Acceptance: same-name/different-lineage packages cannot collide or spoof
  nominal symbols; source/name changes are replacement; exact commit, tree,
  content, toolchain, and evidence identities bind one instance.

  Progress 2026-08-23: a typed identity core now binds `PackageKey` to
  `PackageName` plus `SourceLineage`, and typed immutable source resolutions
  reject a family that does not match the key lineage at graph/source custody
  boundaries. The earlier caller-constructible `PackageInstance` placeholder
  was removed: its replacement must join exact source, toolchain, and sealed
  compiler evidence by construction. Migrating the legacy name-keyed graph,
  lock, and evidence APIs remains. Resolved Git and external-local package
  sources now carry `PackageKey` plus typed immutable source resolution but
  deliberately cannot construct an accepted instance. `PackageKey::identity()`
  now emits a domain-separated opaque 256-bit commitment shared with the compiler;
  it is stable across revisions and changes when package name or canonical
  lineage changes.

- **SOURCE-LINEAGE-NORMALIZATION.** Define canonical lineage for Git, URL
  archives, and local/workspace paths.

  Acceptance: HTTPS/SSH spellings of one known Git repository normalize
  together without asserting equivalence that cannot be established. Mirrors
  require explicit relocation/delegation evidence. Workspace members use
  workspace lineage plus member-relative path; external paths remain marked
  non-portable development sources. Each archive/protocol adapter defines
  lineage and immutable-content evidence instead of guessing from a locator.

  Progress 2026-08-23: the first conservative lineage adapter normalizes known
  GitHub HTTPS, SCP-like SSH, and `ssh://` spellings; unknown hosts retain
  transport, user, port, case-sensitive path, and suffix distinctions.
  Workspace members bind a normalized relative path to workspace lineage, and
  external local sources bind canonical absolute path plus consuming context.
  Archives, mirrors/delegations, additional protocols, and wiring resolver
  receipts into these types remain.

- **PACKAGE-QUALIFIED-NOMINAL-IDENTITY.** Thread `PackageKey` through package,
  symbol, boundary-trait, provider, and evidence identities.

  Acceptance: a same-spelled package or boundary declaration from another
  source lineage cannot satisfy or replace the admitted identity.

  Progress 2026-08-23: target-neutral Psi now owns only the opaque
  `PackageKeyIdentity` carrier, while source-lineage normalization remains in
  `omega-packages`. Managed compiler sources retain that identity and
  same-package checks prefer it over path spelling. Managed authored symbols
  recover it from retained source metadata. Provider plans and provider trust
  rows now retain compiler-derived package identities for the realizing
  machine, nominal provider type, selected service schema, and each inherited
  or direct requirement owner. Those identities enter the existing normalized
  plan fingerprint; readable labels are diagnostic only. That 64-bit
  fingerprint remains review/execution compatibility data, not sealed package
  admission identity. Post-resolution compiler symbols now require an existing
  derivation-origin symbol and inherit its exact package/toolchain provenance;
  source-free symbols remain deliberately unresolved. Checked-adapter rows now
  bind a canonical typed machine-overload identity to the exact package owning
  that machine, reject row transplantation across realizing packages, and
  resolve without short-name fallback in validation, dispatch, progress,
  external-root, TCB, and trust projections. Provider selection identity,
  compiler-intrinsic toolchain identity, terminal Psi, and sealed emitted
  evidence remain.

## P2 — Dependency projection and reconciliation

- **BUILD-DEPENDENCY-API.** Replace the transitional
  `build.depend("alias", path("dir"))` seam with ordinary typed source requests:
  `builder.depend(source)` and exceptional
  `builder.depend_as(alias, source)`.

  Acceptance: normal install supplies only source/revision; package name and
  default alias come from the fetched package. The editor rewrites only
  canonical direct rows and otherwise emits a non-mutating patch. The API is
  implemented with ordinary Omega vocabulary and may be simplified when
  compiler work proves a smaller existing mechanism sufficient.

  Progress 2026-08-23: the ordinary `Source::Path` and `Source::Git` literal
  shapes require no parser syntax. A strict package-side extractor now consumes
  their canonical one-argument `builder.depend(source)` form. The compiler
  prelude and import path still expose the transitional alias/path API;
  `depend_as`, conservative editing, target vocabulary, and orchestration of
  the reconciled compiler bindings remain.

- **HERMETIC-DEPENDENCY-PROJECTION.** Derive dependency source requests without
  executing build-host effects or imported code.

  Acceptance: dependency rows cannot depend on filesystem/network observations,
  generated files, clocks, or build outputs. Malformed or unsupported
  projection rejects explicitly; nothing is silently skipped.

  Progress 2026-08-23: `omega-packages` now parses only the immutable root
  `build.omg`, accepts direct literal Path/Git rows in authored order, and
  rejects authored toolchain vocabulary, malformed/scoped builds, nonliteral or
  nested/helper-mediated requests, unsupported cases, and `depend_as`. An
  absent build machine projects no dependencies. Resolved package-source
  custody performs this projection before returning. The compiler now has
  separate native and checked package-aware entrypoints that accept only a
  validated, closed, requester-local alias-to-`PackageKeyIdentity` graph and
  canonical source roots; this mode never invokes or combines the transitional
  scanner. Recursive package-manager traversal, orchestration wiring, and
  removal of the scanner from legacy standalone compilation remain.

- **CLOSURE-RECONCILIATION.** Resolve the complete source closure before any
  dependency build receives providers.

  Acceptance: one `PackageKey` resolves to one immutable instance in v1;
  conflicts report every requesting dependency path. Resolver authority never
  enters package build execution.

  Progress 2026-08-23: a typed pre-admission source graph now validates exact
  roots and edges, one immutable resolution per `PackageKey`, requester-local
  alias uniqueness, closed reachability, and same-name/different-lineage
  separation. Package dependency cycles conservatively reject in v1. Recursive
  source traversal, request-path provenance, and compiler evidence remain; the
  structural graph has no persistence or admission API. A compiler-side
  handoff independently rejects missing/duplicate/overlapping roots, invalid or
  duplicate requester-local aliases, missing targets, unreachable rows, cycles,
  source-root drift, toolchain overlap, dependency `build.omg` imports, and
  symlink escapes. Translating the package graph into that handoff remains at
  the CLI/orchestration boundary.

## P3 — Compiler-issued package evidence

- **PACKAGE-ADMISSION-COMPILATION.** Add a library/package compilation profile
  independent of executable entry selection.

  Acceptance: the compiler emits source/toolchain-bound evidence for every
  public callable and build machine, including declared and realized reach,
  authority flows, provider realization/provenance, trust/claims, proof status,
  installation rows, operational contracts, executable TCB, observations, and
  reproducibility.

  Progress 2026-08-23: checked trees already own the useful semantic core. A
  `RealizedMachineContractEnvelope` retains contract identity, effective and
  concrete reach, unresolved installation rows, synchronous invocation,
  suspension, blocking, termination, crashes, mutation, and exact capability
  flows. Source-authored symbols can be joined back to their opaque
  `PackageKeyIdentity` through the retained source map, and underdeclared reach
  already fails checking. This is enough for a compiler-owned, target-scoped
  review projection, but not an admission certificate. General `pub`/`export`
  visibility, generated/toolchain symbol ownership, package-qualified provider
  binding/selection identities, source/toolchain/compiler commitments,
  non-provider trust ownership, build observations, and reproducibility
  receipts still need one sealed projection. Exact provenance for the realizing
  package, provider type, service schema, and requirement owner is already
  retained on provider plans and their provider trust rows.
  Until those joins exist, only an authored `boundary machine` is a dependable
  exported-callable classification and no projection may be persisted as
  accepted evidence. The compiler now exposes
  an explicitly review-only, in-memory projection for the reconciled root
  package under an exact target. It retains the selected build-machine symbol,
  package-qualified authored nominals, distinct declared/effective/concrete
  service rows, unresolved installation rows, exact capability-flow
  coordinates, operational outcomes, crashes, mutation, and selected provider
  mechanisms with exact realizing-package, provider-type, service-schema, and
  requirement-owner provenance. Checked-adapter bindings now retain and verify
  canonical overload plus realizing-package identity. Authored provider names
  are resolved once and the exact selected plans are retained through cycle,
  ABI, and checked-fact construction without a name-based candidate rejoin;
  same-spelled selected slots are distinguished by package identity and an
  ambiguous readable invocation target rejects. Authored selector resolution,
  several downstream schema/grant joins, and compiler-intrinsic toolchain
  ownership are not yet package-qualified or sealed. Build-bound progress
  obligations now retain and match the compiler-derived package owners of both
  the provider service and exact requirement, including through component
  manifests and audit rendering; no readable-name lookup remains on retained
  selected-provider facts. Installation-bound reach, termination premises, and
  mutation frames now project package-owned semantic paths rather than
  arena-local handles/row IDs. Crash evidence remains symbol-bearing, so
  canonical machine encoding must still wait; directly serializing the current
  review projection would not be stable across compiler runs.
  Compiler-generated symbols now inherit the exact authored provenance of a
  mandatory derivation origin; truly source-free symbols and exact toolchain
  identity remain visibly unbound rather than guessed.
  Standalone and target-free compilations reject projection.

- **PROOF-AND-BOUNDARY-ADMISSION.** Fail closed on false or incomplete evidence.

  Acceptance: open/deferred proofs reject, checked proofs are kernel-rechecked,
  accepted axioms/opaque claims remain trust-bearing, exact package-qualified
  boundary identities are enforced, underdeclared reach rejects, and dangerous
  overdeclared slack is reported.

  Progress 2026-08-23: concrete proof, contract, bounds, and termination
  obligations normally reject before checked trees are constructed; accepted
  axioms and admitted boundary qualifications remain identifiable. There is no
  implemented open/deferred-proof status yet. Contract entailment deliberately
  stands down for some out-of-engine-language claims. Package-aware checked
  compilation now audits the pristine typed graph, including generic
  templates, and retains exact machine/contract/fact coordinates plus a closed
  reason for every checked-implementation stand-down. The review projection
  rejects any such row; accepted/opaque supply remains in the trust lane rather
  than being mislabeled as an unresolved proof. This is fail-closed review
  behavior, not sealed evidence: terminal propagation, kernel recheck receipts,
  and a possible exact later-discharge ledger remain. Ordinary successful
  compilation is not itself a complete proof verdict. The standalone
  `psi-proof` boundary obligation ledger is not wired into production and must
  not be cited as enforcement.

- **SEALED-EVIDENCE-HANDOFF.** Replace public construction/parsing of
  `PackageCapabilityManifest` as an admission input.

  Acceptance: production orchestration accepts only compiler-issued evidence
  bound to exact source/toolchain identity. A standalone JSON file cannot
  impersonate compiler output.

  Progress 2026-08-23: legacy manifest, lock, whole-section receipt, install,
  update, and graph-audit modules now compile only for isolated crate tests and
  are absent from the release `omega-packages` API. The arbitrary public
  `PackageInstance` plus caller-derived toolchain/evidence fingerprint tuple was
  removed rather than adapted. Source diagnostics were split onto a retained
  production surface. The compiler projection remains explicitly review-only;
  source/toolchain/compiler binding and the remaining completeness joins must
  land before a replacement admission type is issued or persisted.

## P4 — Lock and baseline

- **ACCEPTED-LOCK-SCHEMA.** Replace name-keyed/fingerprint-only lock entries.

  Acceptance: `omega.lock` records `PackageKey`, `PackageInstance`, source
  request and immutable resolution, complete closure, compiler identity,
  normalized accepted capability/API baseline, build observations, provenance,
  and exact conflict-resolution references.

- **LOCK-BASELINE-RECOVERY.** Define missing-state behavior.

  Acceptance: committed lock alone is sufficient for capability comparison;
  unavailable old source triggers standalone source audit; absent accepted lock
  triggers fresh admission of the complete graph; missing normalized evidence
  behind a fingerprint is treated as missing admission evidence.

- **LOCK-CLOSURE-VALIDATION.** Port useful closure/reachability validation to
  `PackageKey` and instance identities.

  Acceptance: duplicate keys, conflicting instances, open edges, unreachable
  rows, stale evidence, and toolchain/source mismatches reject before use or
  persistence.

## P5 — Admission, audit, and review

- **CAPABILITY-CONFLICT-MODEL.** Replace whole-section receipt approval with
  compact row-specific conflicts and exact resolution artifacts.

  Acceptance: conflicts name package/source identity, dependency path, old/new
  checked rows, risk, provenance, and source locations. Every blocking row must
  be resolved; artifacts bind the exact candidate, toolchain, evidence, and
  conflict fingerprint and cannot be issued by the dependency.

- **DANGEROUS-AUTHORITY-CLASSIFICATION.** Classify risk from compiler-owned
  nominal metadata.

  Acceptance: filesystem, network, process, dynamic loading, signing, secrets,
  executable installation, root memory, DMA/IOMMU, interrupts, and equivalent
  authority cannot be spoofed or hidden by package-controlled names.

- **SOURCE-AND-PROVENANCE-TRIAGE.** Run automated/LLM triage for every source
  update, independently of capability equality.

  Acceptance: retained dangerous authority recommends audit; unavailable old
  source escalates to standalone candidate audit; source-lineage/provenance
  changes block as replacement; triage input contains only bounded,
  Omega-rendered evidence and no package prose.

- **AUDIT-RESULT-STATES.** Represent at least `admitted`,
  `admitted-after-audit`, `admitted-with-audit-recommended`,
  `blocked-capability-change`, `blocked-missing-admission-baseline`, and
  `blocked-provenance-change`.

## P6 — Commands

- **OMEGA-INSTALL.** Implement
  `omega install <source> [--rev <revision>] [--as <alias>]`.

  Acceptance: fetch, declaration extraction, closure resolution, compiler
  evidence, conflict handling, and audit all complete before `build.omg` or
  `omega.lock` changes. A failed or unresolved install performs no mutation.

- **OMEGA-UPDATE.** Implement
  `omega update [package-or-alias...] [--to <revision>]`.

  Acceptance: builds from the accepted lock; candidate capability/API change
  blocks; unchanged evidence still receives provenance/source triage; retained
  dangerous authority recommends audit; accepted mutation is atomic.

- **OMEGA-AUDIT-PACKAGES.** Render the accepted graph and current source state.

  Acceptance: output includes source lineage and immutable pins, dependency
  paths, declared/realized reach, authority flow, providers/trust/proofs,
  dangerous slack, build observations, review status, and first failed
  provenance edge.

## P7 — Fixtures

- **MIGRATE-PACKAGE-FIXTURES.** Add `PACKAGE` declarations and canonical build
  variable names to every fixture.

  Acceptance: fixture identity comes from source, not directory names or test
  constructors, and compiler admission emits every expected evidence row.

  Progress 2026-08-23: all eight local package fixtures declare `PACKAGE` and
  use the coherent `builder` parameter name. Their private CathedralOS mirrors
  carry the same declarations at refreshed exact pins. Compiler-issued
  admission evidence remains.

- **SECURITY-FIXTURE-MATRIX.** Add local and remote cases for pure code,
  generated files, filesystem, network overreach, retained filesystem+network
  authority, accepted claims, provider changes, capability flow, missing old
  source, missing lock baseline, same-name/different-lineage spoofing, transport
  normalization, and dependency-version reconciliation conflict.

- [x] **REMOVE-FABRICATED-MANIFEST-TESTS.** Replace integration tests that construct
  manifests from fixture intent with compiler-issued evidence.

  Acceptance: only isolated data-structure unit tests may use synthetic values;
  no end-to-end admission test can pass without compiling the fixture.

  Completed 2026-08-23: the fabricated local fixture admission integration
  test was removed. Synthetic legacy values remain only in isolated unit tests;
  the remote integration suite proves source custody and declarations, not
  package admission.
