# Tasks: Package Manager

Status: remaining work only, 2026-08-28.

This file is the forward queue for the Cargo-like source/package service under
`omega`. Completed milestones live in Git history and in the subsystem notes;
they are deliberately not repeated here.

Governing documents:

- `wiki/design_briefs/package_manager_first_draft.md`
- `wiki/design_briefs/build_and_package_model.md`
- `wiki/language_guide/chapter_15_modules_imports_visibility.md`
- `wiki/language_guide/chapter_19_capabilities_effects_boundaries.md`
- `source/omega-rust/omega/packages/omega-packages/README.md`
- `source/omega-rust/omega/packages/omega-packages/SOURCE_RESOLVER_SECURITY.md`
- `OWNER_QUESTIONS.md`

Do not wire mutating `omega install` or `omega update` until the P0 source
boundary, recheckable evidence, accepted lock, and transaction gates below are
closed. Compiler-issued package review remains non-admitting.

## P0 — Source resolver boundary

- [ ] **PRIVATE-RESOLVER-STORAGE-BY-DEFAULT.** Replace the ambient-filesystem
  custody model with package-manager-owned storage. Ordinary resolution must
  use a private per-user cache and private staging beneath that cache; project
  build products remain beneath the project's own build root. Do not use a
  shared writable cache or ambient `%TEMP%` as the default resolution root.

  Create each root with platform-appropriate private permissions, retain and
  traverse it handle-relatively, authenticate cached content by its immutable
  source identity, and publish through an atomic rename inside the same root.
  Tests must use the production private-root constructor rather than inherit
  the host test runner's temporary-directory ACL.

  Remove unconditional ACL ancestry auditing of ordinary host-installed tools
  and do not maintain an ad hoc allowlist that attempts to reproduce Windows
  trust policy. Resolver tools must be selected explicitly and their exact
  executable identity recorded. A shared cache or stronger executable-custody
  audit may exist only as an explicit hardened/multi-tenant mode with its trust
  policy and resulting evidence disclosed. The normal Windows path must accept
  a standard Git installation under `Program Files` and must not make package
  tests depend on ambient host ACL accidents.

- [ ] **HARDEN-SOURCE-RESOLVER.** Finish the hostile-process boundary around
  local and Git resolution.

  Remaining work:

  - provide Linux filesystem-read/write, executable-path, direct-egress, and
    endpoint confinement rather than relying only on inherited resource limits
    and the cooperative CONNECT broker;
  - provide the corresponding Windows filesystem, network, and executable
    confinement, plus the still-unavailable address-space, file-size,
    descriptor, and core-dump guarantees;
  - run the existing Windows Job Object process-count, per-process memory,
    aggregate-memory, and aggregate-CPU exhaustion pairs on a native Windows
    worker; cross-compilation is not execution evidence;
  - narrow macOS SSH discovery/fetch reads after Q18 settles explicit host-key,
    key, credential-provider, and credential-file custody;
  - enforce whole-operation transfer, object-store, temporary-disk, descendant
    CPU/memory/process, and during-write quotas rather than only rejecting
    oversized retained state after the helper exits;
  - decide and implement the stronger isolation needed against hostile
    same-user cache/source mutation, executable replacement, loaded-image
    substitution, and hostile Unix descendants escaping their process group;
  - replace the current non-admitting resolution observation with a locally
    reconstructed opaque strict receipt binding native enforcement, effective
    endpoints and transfer counts, complete executable custody, exact source
    subjects, and every required resource observation. Missing rows must reject.

  The detailed established floor and remaining platform gaps are maintained in
  `SOURCE_RESOLVER_SECURITY.md`. Strict SSH trust and credential authority is
  design-blocked on OWNER Q18; the other bullets are engineering work.

## P1 — Total package semantic identity

- [ ] **COMPLETE-AUTHORED-SELECTION-CUSTODY.** Finish exact-symbol custody for
  every independently selectable source occurrence that can affect a package
  interface, build behavior, provider choice, proof, or capability result.

  Existing coverage includes ordinary nominal references, calls, operators,
  conformance arguments, `satisfies` coordinates, establishment routes,
  machine-parameter requirements, qualification-cast semantic domains,
  expression-embedded cast/zero-value type references, unary compiler
  intrinsics, and supported member access. Remaining visibility-dependent
  nested positions must either retain exact authored selection and source
  custody or fail before review. Toolchain-authored bodies remain outside
  ordinary package admission.

- [ ] **CLOSE-COMPILER-SEMANTIC-SUBJECTS.** Give every source-free
  compiler-owned semantic subject admitted by package review a closed identity
  selected from exact compiler state, never spelling. The existing closed floor
  covers builtin types, all compiler-installed builtin functions (including
  `min`, `max`, and `sqrt`), unary operators, byte predicates, and collection
  length. Builtin-backed boundary-operator provider rows now retain and
  rederive their exact builtin execution child. Named-float negation provider
  rows likewise retain a closed `f32`/`f64` execution atom independently of
  their authored realization machine. Remaining work includes complete
  source/target identity for named-float conversions, other non-builtin
  intrinsic provider executions, and any source-free child still represented
  as unresolved nominal ownership. Package-authored lookalikes must remain
  ordinary package nominals.

- [ ] **COMPLETE-CONFORMANCE-IDENTITY.** Retain complete public conformance
  applications, including target-trait lifetime arguments once OWNER Q6 is
  settled. Unsupported generic, lifetime-bearing, private, or aliased forms
  must continue to fail closed rather than disappear from review.

## P2 — Total compiler admission projection

- [ ] **PACKAGE-ADMISSION-COMPILATION.** Make the compiler-owned ordinary
  package projection total for the supported language surface after successful
  checking. The canonical output must contain no arena handles, diagnostic
  strings as identity, or compiler-private IDs.

  Remaining projection work includes:

  - advanced call-bearing domain predicates, computed/aggregate contract
    expressions, and structural witness arguments not retained by their owning
    typed or checked representation;
  - remaining semantic-role, operator, selected-provider grant, installation,
    permission-frontier, crash-refinement, and compiler-intrinsic ownership
    joins;
  - an exact trust-bearing association for operator-bound external supply;
  - same-path overloaded boundary-provider selection after OWNER Q10;
  - complete exact semantic-subject commitments, certificate closure, and
    reproducibility dispositions.

  Extend the earliest coherent compiler-owned representation that owns a
  missing fact. Do not reconstruct identity from diagnostics and do not add a
  nominal Chi stage merely to collect private compiler state.

- [ ] **BUILD-OBSERVATION-EVIDENCE.** Generalize the existing exact
  filesystem replay lane into a complete receipted build-operation and output
  grammar. Add replay for every admitted service, exact staged-output
  commitments, failure/denial outcomes, and a complete replay verdict. Enforce
  process CPU/memory and remaining session quotas. A summary or observation
  digest alone is not a receipt.

- [ ] **PROOF-AND-BOUNDARY-ADMISSION.** Locally recheck every proof or retained
  certificate required by an ordinary package claim. Reject open/deferred
  proofs and contract-entailment stand-downs, retain accepted axioms and opaque
  claims as explicit trust rows, and add the later-discharge/open-obligation
  ledger. Do not cite the standalone `psi-proof` ledger as production
  enforcement.

- [ ] **FINAL-REALIZATION-EVIDENCE.** Require exact Terminal evidence only for
  claims about emitted native/external code, ABI/lowering-dependent guarantees,
  fixed native resources, or profiles requesting final-code replay. Keep
  ordinary checked capability/API evidence and opaque executable-supply rows in
  their distinct evidence classes; absence of Terminal evidence grants no
  Terminal claim.

- [ ] **REPRESENTATION-TCB-EVIDENCE.** Replace `Unbound` representation rows
  with exact selected ABI and mechanism evidence when the package makes such a
  claim. Opaque by-value boundary data depends on OWNER Q1. Claim-free opaque
  data must remain review-visible and audit-recommended without fabricating a
  trust claim.

## P3 — Recheckable evidence and accepted lock

- [ ] **PACKAGE-KEY-AND-INSTANCE.** Introduce the final `PackageInstance` only
  after exact source and artifact subjects, obligation-semantics identity,
  locally re-derived discharge results, transitive open assumptions, and root
  admission decisions exist. Do not revive the deleted caller-constructed
  placeholder or treat compiler/toolchain provenance as a seal.

- [ ] **ORDINARY-PACKAGE-ARTIFACT-SUBJECT.** Finish the canonical semantic
  subject for ordinary package claims: one complete versioned row set under the
  exact package key, target, dependency closure, and obligation-semantics
  schema. Source, compiler/process observations, certificates, decisions,
  native code, and Terminal evidence remain separately bound subjects.

- [ ] **RECHECKABLE-PACKAGE-EVIDENCE.** Build the authority-bearing path that
  compiler review deliberately cannot issue. Bind exact requested source,
  produced artifact, obligation schema and locally reconstructed obligation
  set, certificate bundle, derivation provenance, discharge result, and open
  obligations. Compose dependency results and open obligations transitively;
  never compose producer admission decisions.

  Add checked schema-delta handling for unchanged, added, strengthened,
  reinterpreted, retired, and encoding-only classes. Unknown or meaning-changing
  deltas force re-derivation. Missing, stale, dependency-hidden, or
  admission-laundered evidence must reject under local replay.

- [ ] **ACCEPTED-LOCK-SCHEMA.** Define and implement the accepted `omega.lock`
  format over the canonical source-closure question, complete package evidence,
  root decisions, and exact immutable resolutions. The lock must not contain
  compiler-private handles, source cache paths, package-authored verdicts, or a
  compiler/toolchain identity presented as certification.

- [ ] **LOCK-BASELINE-RECOVERY.** Persist and recover accepted baselines with
  strict canonical framing and immediate local reconstruction. Missing lock
  evidence means fresh graph admission; unavailable old source requires a
  standalone candidate audit but does not erase a valid accepted baseline.
  No review-only capsule may be promoted by renaming it.

- [ ] **LOCK-CLOSURE-VALIDATION.** Revalidate exact source lineage,
  resolutions, aliases, dependency reachability, obligation schemas,
  certificates, and open assumptions for the complete closure before any
  accepted lock is used or replaced.

## P4 — Admission policy and review

- [ ] **CAPABILITY-CONFLICT-TRANSACTION.** Integrate row-specific blocking
  conflicts and root-policy dispositions into one locked install/update
  transaction. Reopen and revalidate the accepted lock, candidate closure,
  policy file, and every decision immediately before mutation. Governance
  metadata may be deployment policy; it must not become proof that an audit
  occurred.

- [ ] **DANGEROUS-AUTHORITY-CLASSIFICATION.** Complete exact compiler-owned
  classification for network, dynamic loading, signing, secrets, executable
  installation, DMA/IOMMU, and any future authority-bearing surfaces. Names,
  aliases, paths, and same-spelled package declarations must confer no
  authority. Ordinary std/provider authority depends on OWNER Q7.

- [ ] **SOURCE-AND-PROVENANCE-TRIAGE.** Wire an organization-selected advisory
  reviewer into command orchestration with the existing fixed instructions,
  bounded Omega-rendered evidence, closed response schema, and monotone
  recommendation rule. Apply its result only through root policy. It cannot
  suppress deterministic recommendations, resolve conflicts, admit evidence,
  attest review, or mutate project state.

- [ ] **AUDIT-RESULT-INTEGRATION.** Carry the existing deterministic states—
  admitted, admitted-with-audit-recommended, blocked capability change,
  blocked missing baseline, and blocked provenance replacement—through lock and
  command transactions. Initial install is complete-graph fresh admission, not
  an unchanged update.

## P5 — Commands

- [ ] **OMEGA-INSTALL.** Implement
  `omega install <source> [--rev <revision>] [--as <alias>]` only after P0–P4.
  Fetch, declaration extraction, closure resolution, compiler review,
  recheckable evidence, conflict handling, triage, and root-policy decisions
  must complete before an atomic `build.omg`/`omega.lock` mutation. Failure or
  unresolved review performs no mutation.

- [ ] **OMEGA-UPDATE.** Implement
  `omega update [package-or-alias...] [--to <revision>]` only after P0–P4.
  Resolve from the accepted lock, block capability/API or provenance changes
  pending exact root decisions, recommend audit for retained dangerous
  authority, and publish atomically after final revalidation.

- [ ] **OMEGA-AUDIT-PACKAGES.** Render the accepted graph and current source
  state: immutable lineage/pins, dependency paths, declared and realized reach,
  authority flow, provider/trust/proof state, dangerous slack, build
  observations, review state, and the first failed provenance edge.

- [ ] **OMEGA-FETCH-MEMBER.** After OWNER Q2, add selective authenticated Git
  acquisition for one declared workspace package without using checkout or
  lazy object fetching. Parent-authenticated materialization must prove the
  selected member subtree and every root declaration needed to authenticate
  its membership.

## P6 — Integration fixtures

- [ ] **SECURITY-FIXTURE-MATRIX.** Close the remaining real-custody cases:
  accepted-lock absence and recovery, sealed representation mechanism/ABI,
  canonical network authority, broader receipted build operations and outputs,
  final native transaction publication, and credential-gated remote mirrors.
  Synthetic end-to-end security artifacts are not permitted.

- [ ] **WINDOWS-RESOLVER-CANARIES.** Run the compiled Job Object exhaustion
  controls and negative cases on a native Windows worker and retain the results
  in the normal test lane.

- [ ] **PRIVATE-REMOTE-FIXTURES.** Run the exact pinned CathedralOS SSH/HTTPS
  mirror tests in credentialed infrastructure. Unavailable credentials must
  remain an explicit ignored/blocked environment condition, never a fallback
  to fabricated or ambient evidence.

## P7 — Owner-blocked package work

- [ ] **BLOCKED — OWNER Q1: OPAQUE-BY-VALUE-BOUNDARY-ABI.** Settle how a
  selected provider supplies the target-specific representation descriptor for
  opaque boundary data passed by value before package review can replace its
  `Unbound` ABI/mechanism row.

- [ ] **BLOCKED — OWNER Q2: MULTI-PACKAGE-GIT-SELECTION.** Add an explicit Git
  package selector for workspace repositories. The selected member's own
  `builder.package("name")` remains identity authority; the request string is
  selection intent only. This blocks remote `omega-language-std` selection and
  selective member fetch.

- [ ] **BLOCKED — OWNER Q3/Q8: PACKAGE-NATIVE-GENERATED-SOURCE-TRANSACTION.**
  Route native-image production through the sponsored package transaction
  without rerunning `build.omg` or reopening discovery. Lower the exact frozen
  checked program after generated-source handoff, retain the unpublished native
  artifact as an exact subject, reconstruct every source/build/generated/native
  commitment, and publish only after complete accepted comparison. Q3 must
  settle application-root identity and Q8 requested-versus-source-selected
  target identity first.

- [ ] **BLOCKED — OWNER Q4: SCOPED-BUILD-ROOTS.** Retire the five remaining
  `Owner::build` compatibility canaries or formally admit one shared scoped-root
  grammar. Package readers and standalone compilation may not continue assigning
  different meanings to the same `build.omg` shape.

- [ ] **BLOCKED — OWNER Q6: CONFORMANCE-TARGET-LIFETIMES.** Settle and retain
  the complete target-trait lifetime application before lifetime-parameterized
  public conformances can enter canonical package identity.

- [ ] **BLOCKED — OWNER Q7: ORDINARY-STD-AND-PROVIDER-AUTHORITY.** Replace all
  physical `source/library/std` routing and direct filesystem/GUI provider
  injection with exact ordinary graph nodes and explicit authenticated role
  bindings. Only core remains compiler-welded. Removing the declared std edge
  must reject every std selection; no package name, alias, path, or magic mount
  may confer authority.

- [ ] **BLOCKED — OWNER Q10: OVERLOADED-BOUNDARY-PROVIDER-SELECTION.** Settle
  authored override selection for same-path overloaded boundary-operator
  families before admitting that provider form into package evidence.

- [ ] **BLOCKED — OWNER Q18: STRICT-SSH-CUSTODY.** Settle host-key, key,
  credential-provider, and credential-file authority before narrowing the
  remaining SSH read surface or treating SSH resolution as strict evidence.

## P8 — Final release gate

- [ ] **PACKAGE-MANAGER-RELEASE-AUDIT.** Before enabling mutation, rerun the
  complete package, package-review, package-compilation, resolver, compiler
  handoff, platform-native, fixture, recovery, and architecture suites. Require
  zero unexpected ignores, no legacy manifest/receipt/plan surface, no physical
  std authority, no unresolved canonical evidence rows, and a clean atomic
  failure path for every install/update stage.
