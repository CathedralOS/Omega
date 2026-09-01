# Tasks: Package Manager

Status: remaining work only, 2026-08-30.

This file is the forward queue for the Cargo-like source/package service under
`omega`. Completed milestones live in Git history and in the subsystem notes;
they are deliberately not repeated here.

Reference documents (the first-draft brief is non-normative where it conflicts
with the current build/package model or subsystem contracts):

- `wiki/design_briefs/package_manager_first_draft.md`
- `wiki/design_briefs/build_and_package_model.md`
- `wiki/language_guide/chapter_15_modules_imports_visibility.md`
- `wiki/language_guide/chapter_19_capabilities_effects_boundaries.md`
- `source/omega-rust/omega/packages/README.md`
- `source/omega-rust/omega/packages/sources/acquisition/SOURCE_RESOLVER_SECURITY.md`
- `OWNER_QUESTIONS.md`

Do not enable a mutating `omega install` or `omega update` path until its exact
candidate closure can produce recheckable evidence, accepted-lock rows, root
decisions, and one atomic transaction. Unsupported language/build forms reject
that candidate; they do not globally block commands for unrelated closures.
Compiler-issued package review remains non-admitting.

Security work must name a concrete invariant Omega can enforce inside compiler,
package, or artifact custody. Do not expand this subsystem into operating-system
policy, operator custody, unverifiable review ceremony, or proxy metrics
presented as containment. If that authority boundary is genuinely ambiguous,
stop the item on one precise owner question before adding machinery.

## P2 — Compiler review and realization projection

- [ ] **PACKAGE-REVIEW-PROJECTION.** Make the compiler-owned ordinary package
  projection total for the supported language surface after successful
  checking. The canonical output must contain no arena handles, diagnostic
  strings as identity, compiler-private IDs, or compiler-issued admission
  verdicts.

  Remaining work:

  - finish D29 artifact-qualified symbolic demand/final substitution for
    separately compiled generic artifacts, nested calls, and remaining
    supported operator categories;
  - complete D32 physical-child custody for verified non-identity optimization
    and remaining native roles, with an exact bijection to surviving optimized
    occurrences and reconstructible D29/D41 parents;
  - enable the checked-body physical lane without weakening the existing
    `InvalidLinuxExitGroupShape` backend rejection; and
  - add external realization custody only when independently admitted concrete
    authority exists. Never substitute a self-issued commitment.

  Unsupported telescope/application forms remain fail-closed. Add a fact at
  the earliest coherent compiler-owned representation; do not reconstruct it
  from diagnostics or add a nominal stage only to collect private state. D46
  continues to forbid producer-executable path bytes as review, conflict,
  lock, or admission identity.

- [ ] **FINAL-REALIZATION-EVIDENCE.** Require exact Terminal evidence only for
  claims about emitted native/external code, ABI/lowering-dependent guarantees,
  fixed native resources, or profiles requesting final-code replay. Keep
  ordinary checked capability/API evidence and opaque executable-supply rows in
  their distinct evidence classes; absence of Terminal evidence grants no
  Terminal claim.

  Complete the remaining checked-body D29 physical lane, admitted-provider
  parent, and verified non-identity optimization-projection lanes. Extend
  `CompilerBuiltinExecution` only for a demanded local target mechanism and
  keep planner conversion exhaustive.
  Retain complete standalone-product structures as additional native proposal
  classes land; do not regress to hidden `CheckedCompilation` state or replace
  those structures with compact report fingerprints. The accepted package
  assembler consumes the resulting gate under
  `PACKAGE-NATIVE-GENERATED-SOURCE-TRANSACTION`.

- [ ] **REPRESENTATION-TCB-EVIDENCE.** Add D26 consumer demand only for an
  actual runtime by-value use, consuming the exact retained
  `BoundaryOpaqueRepresentationUse` and validated calling-plan realization
  rather than reconstructing either from an aggregate digest. Rejoin the exact
  boundary requirement application, opaque declaration, named conformance or
  compiler-owned target-semantics application, carrier, selected immutable
  producer source, closed shape graph, physical movement and role-tagged
  lifecycle disposition,
  target/representation version, evidence origin, closed-conformance
  commitment, and complete boundary-plan commitment. Keep selecting-build
  occurrence/source custody as provenance outside ABI comparison. Checked
  carrier derivation is recheckable evidence; foreign representation supply
  remains a disclosed admission.
  Claim-free opaque data stays review-visible without fabricating a
  proposition, minting authority, or service reach claim.

  Carry the retained selected copy receipt into eventual D26 consumer demand
  and complete target-ABI movement closure. Do not publish a partial demand row
  from the copy receipt, calling-convention shape, or size/alignment alone. Add
  canaries proving that
  independently reviewed dependencies may retain different historical
  selections while one later source consumer selects its own application. The
  future `PackageInstance` composition canary must reject unequal commitments
  only at an actual by-value exchange.

## P3 — Recheckable evidence and accepted lock

Dependency order is strict here. Finish the authority-bearing evidence classes
required by the supported package surface, then construct `PackageInstance`,
then implement the accepted-lock codec. The current
`AcceptedOrdinaryClosureEvidence` is an in-memory gate for its explicitly open
ordinary lanes; encoding that subset would create a partial lock and is
forbidden. `LOCK-BASELINE-RECOVERY` and `LOCK-CLOSURE-VALIDATION` begin only
after the complete current-version lock payload exists.

- [ ] **PROOF-AND-BOUNDARY-ADMISSION.** Complete the authority-bearing later-
  discharge/open-obligation result and locally recheck every retained
  certificate required by an ordinary package claim.
  Remaining work is concrete compiler-owned certificate/discharge classes and
  required final-realization joins. Preserve exact `OpenLaterDischarge`
  propagation and pre-policy rejection. Do not persist this partial lane, cite
  standalone `psi-proof` as production enforcement, or add an empty generic
  certificate framework.

- [ ] **PACKAGE-KEY-AND-INSTANCE.** Introduce the final `PackageInstance` only
  after exact source and artifact subjects, obligation-semantics identity,
  locally re-derived discharge results, transitive open assumptions, and root
  admission decisions exist. Do not revive the deleted caller-constructed
  placeholder or treat compiler/toolchain provenance as a seal.

- [ ] **RECHECKABLE-PACKAGE-EVIDENCE.** Build the authority-bearing path that
  compiler review deliberately cannot issue. Bind exact requested source,
  produced artifact, obligation schema and locally reconstructed obligation
  set, certificate bundle, derivation provenance, discharge result, and open
  obligations. Compose dependency results and open obligations transitively;
  never compose producer admission decisions. Implement concrete
  certificate-bearing result classes when the compiler owns them,
  final-realization artifact joins where required, and the persistable complete
  evidence form; do not add an empty generic certificate framework in
  anticipation of those lanes.

  Apply **Accepted locks are current-version generated artifacts**. Require
  exact semantic-schema identity; a mismatch receives complete local
  reconstruction and fresh admission rather than reuse of old discharge or
  policy decisions. Missing, stale, dependency-hidden, or admission-laundered
  evidence rejects under local replay. There is no semantic-schema migration
  registry or compatibility classifier.

- [ ] **ACCEPTED-LOCK-SCHEMA.** Define and implement the accepted `omega.lock`
  format over the canonical source-closure question, complete package evidence,
  root decisions, and exact immutable resolutions. The lock must not contain
  compiler-private handles, source cache paths, package-authored verdicts, or a
  compiler/toolchain identity presented as certification. Begin with fixed
  magic and one outer accepted-lock format version checked before payload
  allocation or interpretation. That version covers the complete payload
  contract, including every nested schema and encoding required for acceptance;
  an incompatible nested change therefore bumps it. Decode only the current
  version and reject unknown versions with regeneration guidance.
  **Blocked on:** the required P2 evidence lanes,
  `RECHECKABLE-PACKAGE-EVIDENCE`, and `PACKAGE-KEY-AND-INSTANCE`. Do not land an
  outer frame, magic-only file, or codec over
  `AcceptedOrdinaryClosureEvidence` while those inputs are incomplete.

- [ ] **LOCK-BASELINE-RECOVERY.** Persist and recover accepted baselines with
  strict canonical framing and immediate local reconstruction. Missing lock
  evidence means fresh graph admission. Unavailable old source produces a
  standalone-candidate review packet and audit recommendation; it neither
  proves an audit occurred nor erases a valid accepted baseline. No review-only
  capsule may be promoted by renaming it. An unsupported historical lock is
  retained opaquely for its matching old toolchain or separate audit tooling;
  current Omega does not migrate or grandfather its acceptance.
  **Blocked on:** `ACCEPTED-LOCK-SCHEMA`.

- [ ] **LOCK-CLOSURE-VALIDATION.** Revalidate exact source lineage,
  resolutions, aliases, dependency reachability, obligation schemas,
  certificates, and open assumptions for the complete closure before any
  accepted lock is used or replaced.
  **Blocked on:** `ACCEPTED-LOCK-SCHEMA` and
  `LOCK-BASELINE-RECOVERY`.

## P4 — Admission policy and review

- [ ] **CAPABILITY-CONFLICT-TRANSACTION.** Integrate row-specific blocking
  conflicts and root-policy dispositions into one locked install/update
  transaction. Reopen and revalidate the accepted lock, candidate closure,
  policy file, and every decision immediately before mutation. Governance
  metadata may be deployment policy; it must not become proof that an audit
  occurred. Do not add another review receipt in place of accepted-lock
  reopen/revalidation and the atomic install/update transaction.

- [ ] **AUDIT-RESULT-INTEGRATION.** Carry the existing deterministic states—
  no review blocker, no review blocker with audit recommended, blocked
  capability change, blocked missing baseline, and blocked provenance
  replacement—through lock and command transactions. Initial install is
  complete-graph fresh admission, not an unchanged update.

## P5 — Commands

- [ ] **OMEGA-INSTALL.** Implement
  `omega install <source> [--rev <revision>] [--as <alias>]` once the selected
  candidate can complete the required P2–P4 gates.
  Fetch, declaration extraction, closure resolution, compiler review,
  recheckable evidence, conflict handling, deterministic triage, and root-policy
  decisions must complete before an atomic `build.omg`/`omega.lock` mutation.
  An advisory reviewer is optional and is not an availability dependency.
  Failure, a blocking conflict, or a missing required root decision performs no
  mutation. An audit recommendation is non-blocking unless external project
  policy makes it blocking.

- [ ] **OMEGA-UPDATE.** Implement
  `omega update [package-or-alias...] [--to <revision>]` once the selected
  candidate can complete the required P2–P4 gates. Resolve from the accepted
  lock, block exact blocking-row changes and declared-name/source-lineage
  replacement pending root decisions, render other typed provenance drift as
  review evidence, recommend audit for retained dangerous authority, and
  publish atomically after final revalidation.

- [ ] **OMEGA-AUDIT-PACKAGES.** Render the accepted graph and current source
  state: immutable lineage/pins, dependency paths, declared and realized reach,
  authority flow, provider/trust/proof state, dangerous slack,
  admission-relevant build replay evidence, deterministic audit
  recommendations, exact root-policy dispositions, and the first failed
  provenance edge. Exclude source-helper execution telemetry, and never render
  reviewer metadata as evidence that an audit occurred or was serious.

## P6 — Source integration and fixtures

- [ ] **WINDOWS-RESOLVER-CANARIES.** Run the compiled Job Object exhaustion
  controls and negative cases on a native Windows worker and retain the results
  in the normal test lane.

## P7 — Cross-system package work

These tasks consume settled language and architecture decisions across package,
compiler, and runtime owners. A task that still needs an owner decision says so
explicitly.

- [ ] **OPAQUE-BY-VALUE-BOUNDARY-ABI — propagate the selected application.**
  - [ ] complete physical movement/lifecycle planning, including D44's
    transitive inert-carrier proof and multiplicity checks;
  - **IMPLEMENTATION — D26 representation application attribution:** publish
    consumer demand from retained compiler custody, rejoin foreign demand to
    landed producer-availability rows and immutable source, and preserve strong
    application equality at actual independently compiled by-value composition
    edges;
  - bind the application into artifacts, replacement compatibility, stable-
    handle era rules, and independently replaceable provider contracts;
  - add compiler-sealed `Ptr<T>` target-semantic closure plus proof-only `Real`,
    `EfiSystemTable`, provider drift, replay drift, and cleanup/multiplicity
    canaries.

- [ ] **D54-EXPLICIT-MULTI-TARGET-ORCHESTRATION — fan out only where target
  semantics begin.**
  - accept one nonempty caller-supplied set of exact target profiles; normalize
    it to canonical profile order and reject `all`, `*`, empty, inferred, or
    dependency-expanded target sets;
  - acquire and retain one immutable source snapshot and reuse parsing, flat
    build facts, and every other target-independent stage result;
  - project one independently valid exact-target child at the first
    target-sensitive stage, preserving the same child subject and identity as
    a standalone invocation;
  - reuse an identical checked/Terminal Psi or PCC product across target
    children only after exact strong-identity equality, then supply each native
    branch with its own target and lowering-authority inputs;
  - retain one batch manifest over the explicit request set and child outcomes,
    without publishing application-support, tested-target, audit, or deployment
    coverage; and
  - add one-target/many-target equivalence, sibling-identity stability, shared-
    Psi/different-lowerer, target-specific-Psi, continue-after-child-failure,
    and forbidden-discovery controls.

- [ ] **APPLICATION-ROOT-ROLE-EVIDENCE — retain the admitted root role through
  authority-bearing outputs.**
  - retain `{ PackageKey, BuildDeclarationKind }` through accepted lock rows,
    command diagnostics, and audit output;
  - add package/application replay, tampering, and role-change fixtures as each
    accepted-lock, command, and audit boundary lands.

- [ ] **PACKAGE-NATIVE-GENERATED-SOURCE-TRANSACTION.**
  Route native-image production through the sponsored package transaction
  without rerunning `build.omg` or reopening discovery. Lower the exact frozen
  checked program after generated-source handoff, retain the unpublished native
  artifact as an exact subject, reconstruct every source/build/generated/native
  commitment, and publish only after complete accepted comparison. Consume the
  retained application-root `PackageKey` and role above; exact requested-target
  identity comes from **IMMUTABLE-TARGET-ACTIVATION-AND-REACH-CLOSURE** in
  `TASKS.md`.

- [ ] **OPTIONAL-STDLIB-BUILD-PROTOCOL-AND-SEMANTIC-BINDINGS.** Finish the
  ordinary-package std migration without recreating a privileged `std` role.
  Only core and genuinely compiler-injected vocabulary remain toolchain-owned;
  std may be replaced, split, or absent. Removing its graph edge must reject
  every std import or provider selection, and no name, alias, path, repository,
  filename, or same-spelled declaration may restore it.

  Remaining work:

  - migrate remaining package-aware fixtures to explicit std dependency edges;
    freestanding UEFI package roots remain dependency-free, and standalone
    source fixtures stay on the compatibility path until they acquire real
    package roots;
    current trait-fixture compatibility seams are the two synthesized
    trait-default roots whose copied call token still loses its original
    requirement identity and the two local named-`dyn` roots rejected by the
    native LET-receiver realization fence; the two build/runtime float twins
    retain compatibility because early named-operator calls have no exact
    operational callable identity, and the x86 FMA plan-association root does
    so because targetless package dependency projection omits the exact-target
    `Build.x86_deployment_features` field; six nested/repeated wire roots retain
    compatibility because generated codec source loses requester-owned schema
    type visibility under package-scoped compilation; exact visible requirement
    identity now survives normalized float-builtin settlement, but three
    arithmetic float helper roots retain compatibility because their scalar
    setup and control roots have no Terminal composed-Unit plan;
    three call roots retain compatibility for transition-argument operator
    finalization, inline subslice-member finalization, and borrow-liveness
    across an owned receiver call;
  - replace the remaining standalone std/alloc `Toolchain` compatibility
    classification only after every compiler consumer has an exact
    source-byte catalog entry or accepted semantic role; a new label derived
    from directory location is not a security boundary;
  - feed every accepted semantic binding, including Console, Filesystem, and
    UEFI, through lock replay into normal package-aware compilation;
    **Blocked on:** `ACCEPTED-LOCK-SCHEMA`, `LOCK-BASELINE-RECOVERY`, and
    `LOCK-CLOSURE-VALIDATION`. Do not create a partial lock codec to move them.

  Do not substitute a package name, alias, repository, path, filename, or bare
  `PackageKeyIdentity` for an exact accepted binding. Do not expand Build facets
  without a concrete package-build consumer.

- [ ] Consume **TOP-LEVEL-BOUNDARY-REQUIREMENTS** from `TASKS.md` after its
  canonical compiler work lands: make selected package invocation replay the
  installed execution and era. Do not synthesize a requirement or satisfier
  edge from equal reach, bodylessness, catalog presence, or build policy.

## P8 — Final release gate

- [ ] **PACKAGE-MANAGER-RELEASE-AUDIT.** Before enabling mutation, rerun the
  complete package, package-evidence, package-compilation, resolver, compiler
  handoff, platform-native, fixture, recovery, and architecture suites. Define
  the exact expected-ignore allowlist and retired surfaces, require no physical
  std special-casing or unresolved canonical evidence rows, and verify a clean
  atomic failure path for every install/update stage.
