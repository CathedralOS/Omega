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

  Progress 2026-08-23: diagnostic source commands now require an explicit
  `local` or `git` adapter; unknown URLs are no longer guessed to be Git. Local
  source identity now uses injective versioned framing over raw relative-path
  bytes, entry kind, symlink target spelling, Unix executable mode, length, and
  content, rejects special files, and checks file limits before allocation.

  Known suspect points:

  - the cache directory uses only a truncated locator/revision hash and reuses
    an existing repository without proving its origin;
  - Git inherits system/user configuration, filters, hooks, credential helpers,
    SSH configuration, and transport subprocess behavior;
  - checkout happens before all repository policy has been validated;
  - source hashing lossily converts non-UTF-8 paths and omits file type, symlink
    identity, and mode semantics;
  - local source hashing has no immutable snapshot/TOCTOU boundary, includes
    tool-owned build outputs when present, and shared Git checkouts can be
    mutated by concurrent resolution;
  - every non-local locator is currently treated as Git, so transport/source
    kind is guessed rather than selected and receipted;
  - resolver process/network/filesystem authority is not yet represented by a
    hardened execution boundary and receipt.

  Acceptance: cache ownership/origin is verified, identities use full
  collision-resistant keys, Git runs with sealed configuration in an isolated
  process boundary, checkout/archive policy is enforced before consumption,
  and source hashing is injective over every filesystem distinction that can
  affect compilation or build execution.

## P1 — Package declaration and identity

- **PACKAGE-DECLARATION-VOCABULARY.** Add the toolchain-owned `Package` build
  data and require exactly one `const PACKAGE: Package` in each package
  `build.omg`.

  Acceptance: extraction occurs hermetically before dependency resolution or
  build execution and rejects missing, duplicate, effectful, generated,
  dependency-dependent, or invalidly spelled declarations.

- **PACKAGE-KEY-AND-INSTANCE.** Replace name-keyed graph and lock APIs with
  `PackageKey` and `PackageInstance`.

  Acceptance: same-name/different-lineage packages cannot collide or spoof
  nominal symbols; source/name changes are replacement; exact commit, tree,
  content, toolchain, and evidence identities bind one instance.

- **SOURCE-LINEAGE-NORMALIZATION.** Define canonical lineage for Git, URL
  archives, and local/workspace paths.

  Acceptance: HTTPS/SSH spellings of one known Git repository normalize
  together without asserting equivalence that cannot be established. Mirrors
  require explicit relocation/delegation evidence. Workspace members use
  workspace lineage plus member-relative path; external paths remain marked
  non-portable development sources. Each archive/protocol adapter defines
  lineage and immutable-content evidence instead of guessing from a locator.

- **PACKAGE-QUALIFIED-NOMINAL-IDENTITY.** Thread `PackageKey` through package,
  symbol, boundary-trait, provider, and evidence identities.

  Acceptance: a same-spelled package or boundary declaration from another
  source lineage cannot satisfy or replace the admitted identity.

## P2 — Dependency projection and reconciliation

- **BUILD-DEPENDENCY-API.** Replace the transitional
  `build.depend("alias", path("dir"))` seam with ordinary typed source requests:
  `build.depend(source)` and exceptional `build.depend_as(alias, source)`.

  Acceptance: normal install supplies only source/revision; package name and
  default alias come from the fetched package. The editor rewrites only
  canonical direct rows and otherwise emits a non-mutating patch.

- **HERMETIC-DEPENDENCY-PROJECTION.** Derive dependency source requests without
  executing build-host effects or imported code.

  Acceptance: dependency rows cannot depend on filesystem/network observations,
  generated files, clocks, or build outputs. Malformed or unsupported
  projection rejects explicitly; nothing is silently skipped.

- **CLOSURE-RECONCILIATION.** Resolve the complete source closure before any
  dependency build receives providers.

  Acceptance: one `PackageKey` resolves to one immutable instance in v1;
  conflicts report every requesting dependency path. Resolver authority never
  enters package build execution.

## P3 — Compiler-issued package evidence

- **PACKAGE-ADMISSION-COMPILATION.** Add a library/package compilation profile
  independent of executable entry selection.

  Acceptance: the compiler emits source/toolchain-bound evidence for every
  public callable and build machine, including declared and realized reach,
  authority flows, provider realization/provenance, trust/claims, proof status,
  installation rows, operational contracts, executable TCB, observations, and
  reproducibility.

- **PROOF-AND-BOUNDARY-ADMISSION.** Fail closed on false or incomplete evidence.

  Acceptance: open/deferred proofs reject, checked proofs are kernel-rechecked,
  accepted axioms/opaque claims remain trust-bearing, exact package-qualified
  boundary identities are enforced, underdeclared reach rejects, and dangerous
  overdeclared slack is reported.

- **SEALED-EVIDENCE-HANDOFF.** Replace public construction/parsing of
  `PackageCapabilityManifest` as an admission input.

  Acceptance: production orchestration accepts only compiler-issued evidence
  bound to exact source/toolchain identity. A standalone JSON file cannot
  impersonate compiler output.

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

- **SECURITY-FIXTURE-MATRIX.** Add local and remote cases for pure code,
  generated files, filesystem, network overreach, retained filesystem+network
  authority, accepted claims, provider changes, capability flow, missing old
  source, missing lock baseline, same-name/different-lineage spoofing, transport
  normalization, and dependency-version reconciliation conflict.

- **REMOVE-FABRICATED-MANIFEST-TESTS.** Replace integration tests that construct
  manifests from fixture intent with compiler-issued evidence.

  Acceptance: only isolated data-structure unit tests may use synthetic values;
  no end-to-end admission test can pass without compiling the fixture.
