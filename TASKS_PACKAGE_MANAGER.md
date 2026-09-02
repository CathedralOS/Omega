# Tasks: Package Manager

Status: remaining work only, 2026-09-01.

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

  - finish D29 cross-artifact final substitution for separately compiled
    generic artifacts. The producer-side package projection now retains a
    public generic callable's direct type-binder boundary demand under exact
    package-qualified operator/callable identity, without claiming coverage;
    `PackageInstance` composition must rejoin a foreign reachable
    specialization and close every symbolic argument before coverage. Extend
    the landed local
    final-substitution fixed point beyond the proven type/const
    single-instantiation scalar-helper, nested-expression, and
    selected-provider-chain cohort only as another concrete language form
    requires it;
  - extend the landed verified Psi-phase D32 non-identity projection through
    selected-lowering, allocation, post-allocation, and layout optimization.
    Extend admitted-provider D41 custody beyond the landed normalized-import
    lane with fixed-width integer scalar arguments/results and one direct
    compiler-private callback parameter to structural arguments/results,
    ranked control, and port-bearing artifacts. Preserve the exact
    survivor/child bijection and reconstructible D29/D41 parents. For the
    structural lane, start with one source-rooted canary whose owned,
    unrestricted flat-record argument reaches a normalized import through a
    checked Unit plan; current natural receiver/forwarding shapes stop before
    Terminal construction. Extend that earliest checked custody first, then
    carry the same argument through lowering and physical replay. Do not land
    a backend-only structural carrier that no Omega source can exercise;
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

  Complete the wider admitted-provider call forms and post-Psi optimization-
  projection lanes. Extend `CompilerBuiltinExecution` only for a demanded local
  target mechanism and keep planner conversion exhaustive.
  Retain complete standalone-product structures as additional native proposal
  classes land; do not regress to hidden `CheckedCompilation` state or replace
  those structures with compact report fingerprints. The accepted package
  assembler consumes the resulting gate under
  `PACKAGE-NATIVE-GENERATED-SOURCE-TRANSACTION`.

- [ ] **REPRESENTATION-TCB-EVIDENCE.** Complete D26 at independently compiled
  artifact composition. Rejoin a foreign consumer demand to the producer's
  reviewed opaque/conformance/carrier declarations and immutable resolved
  source instance, then reject unequal strong application commitments only at
  an actual by-value exchange. Add the corresponding `PackageInstance`
  composition and independently reviewed historical-selection canaries.
  Extend the landed named-conformance demand vocabulary only when a real
  compiler-owned target-semantics application, replacement contract, or
  stable-handle era requires another closed case; do not infer one from
  size/alignment, compact fingerprints, or review prose.

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
  The first compiler-owned class now kernel-checks an exact authored `ensures`
  fact discharged by an identical immutable-scalar machine `requires`
  assumption. Package evidence matches it to the stable reviewed callable,
  coordinates, and strong contract commitment, independently rechecks it, and
  emits a concrete in-memory discharged result; missing or duplicate evidence
  remains open or rejects, and an end-to-end dependency canary proves only the
  discharged row ceases to propagate as `OpenLaterDischarge`; both open and
  discharged results now retain explicit transitive root-closure indexes with
  their original package owners. Remaining work is to add any further classes
  demanded by the supported package surface and finish required final-
  realization joins. Preserve exact open-obligation propagation and pre-policy
  rejection. Do not persist this partial lane, cite standalone `psi-proof` as
  production enforcement, or add an empty generic certificate framework.

  **Landed prerequisite:** `TASKS.md` `PROOF-CERTIFICATION-BRIDGE` owns the
  first real checked-IR assumption-discharge certificate and local rechecker,
  and package evidence consumes that exact compiler product. Current Terminal
  modules still do not represent authored source stand-down goals, so a
  coordinate sidecar must not pretend to establish other discharge classes.

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
  - rejoin landed consumer demand to foreign producer-availability rows and
    immutable source, and preserve strong application equality at actual
    independently compiled by-value composition edges;
  - bind the application into artifacts, replacement compatibility, stable-
    handle era rules, and independently replaceable provider contracts;
  - add compiler-sealed `Ptr<T>` target-semantic closure plus proof-only `Real`,
    `EfiSystemTable`, provider drift, replay drift, and cleanup/multiplicity
    canaries.

- [ ] **APPLICATION-ROOT-ROLE-EVIDENCE — retain the admitted root role through
  authority-bearing outputs.**
  - retain `{ PackageKey, BuildDeclarationKind }` through accepted lock rows,
    command diagnostics, and audit output;
  - add package/application replay, tampering, and role-change fixtures as each
    accepted-lock, command, and audit boundary lands.

- [ ] **PACKAGE-NATIVE-GENERATED-SOURCE-TRANSACTION — finish accepted
  publication around retained reviewed production.**
  - join the retained unpublished native artifact to complete recheckable
    package evidence and accepted-lock state after the remaining P2 authority
    and final-realization lanes close;
  - consume the application-root `PackageKey`, authored role, and exact
    requested-target identity from
    **IMMUTABLE-TARGET-ACTIVATION-AND-REACH-CLOSURE** in `TASKS.md`; and
  - publish only after exact source/build/generated/native comparison and
    `PackageInstance` construction succeed.

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
    package roots. The remaining compatibility seams are:
    - two synthesized trait-default roots now retain separate authored-
      requirement, template-application, and executable-realization custody;
      their remaining target-neutral gap is a composed-Unit plan for scalar
      arguments and structural-field mutation. Their native
      canaries additionally depend on `OWNER_QUESTIONS.md` Q2 because the
      mutation must remain observable through the caller's mutable structural
      parameter; a staged by-value copy is not a realization of that identity;
    - the named-`dyn`/Console ordinary-package native canary, still needing
      accepted Console semantic-binding replay from the future lock and
      multi-block target continuation. The existing consumer-scoped Console
      admission path now crosses package-qualified Fused root establishment
      into a validated unpublished native artifact;
    - two build/runtime float twins whose exact named-operator identity now
      survives early build-time selection and call-closure authority, but whose
      runtime entries still lack a Terminal composed-Unit plan for scalar
      setup, indexed assignment, and guarded Console leaves;
    - six nested/repeated wire roots whose package-aware schema identity and
      public `FixedVec` carrier surface are now closed, but whose runtime
      entries all still lack a Terminal composed-Unit plan for their attached
      transitive machine closure;
    - three arithmetic float-helper roots lacking a Terminal composed-Unit plan
      for scalar setup and control; and
    - three call roots: the guarded transition-argument root, whose copied
      authored operators now retain exact source provenance through checked
      selection finalization but which still lacks a Terminal composed-Unit
      plan for its attached closure; the inline
      subslice-member root, whose checked and Terminal composed-Unit custody is
      now closed but whose ordinary-package native path still reaches the
      deliberately Linux-only sealed `Console::exit_process` physical catalog
      on non-Linux hosts; and the looping-cast root, whose constrained `u8`
      argument no longer manufactures owned-transfer conflicts but which still
      lacks a Terminal composed-Unit plan spanning borrowed-view setup, looping
      scalar dispatch/result conversion, and guarded Console leaves;
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
