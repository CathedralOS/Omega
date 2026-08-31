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

## P2 — Compiler admission projection

- [ ] **PACKAGE-ADMISSION-COMPILATION.** Make the compiler-owned ordinary
  package projection total for the supported language surface after successful
  checking. The canonical output must contain no arena handles, diagnostic
  strings as identity, or compiler-private IDs.

  Remaining projection work includes:

  - canonical const values beyond the completed public named acyclic,
    monomorphic checked-record/pure-sum cohort, and closed conformance static
    applications beyond the completed lifetime-free cohort whose conformance
    telescope contains exact types or parser-canonical integer-literal consts
    and whose target-trait telescope is type-only, but
    only where successful checking retains an exact structural value and call-
    occurrence witness. Direct named public integer and Boolean const values,
    checked structured values with nested literal arrays, forwarded caller
    const binders, and the first exact closed-conformance occurrence lane are
    complete;
  - generic or lifetime-parameterized external/top-level realizations,
    unsupported compiler-intrinsic execution identities, and the remaining
    provider-demand and installation-issuance joins;
  - finish D29's artifact-qualified symbolic demand and final substitution,
    role-specific checked-realization replay, and D32's exact optimized-
    projection-to-physical-child join. Each physical child must bind its exact
    Terminal parent and surviving optimized operation occurrence; the complete
    child set must equal the derived surviving set. Lifetime, machine, and
    proposition operator applications remain fail-closed. D28 authorizes no
    universal generic coverage; only compiler-derived demand joined to an
    independently checked application-specific realization may authorize a
    row;
  - complete exact semantic-subject commitments, certificate closure, and
    reproducibility dispositions.

  Extend the earliest coherent compiler-owned representation that owns a
  missing fact. Do not reconstruct identity from diagnostics and do not add a
  nominal Chi stage merely to collect private compiler state.
  Do not create package-review work for forms the language rejects: proposition
  parameters are trait-only, proof-static evidence cannot eliminate into an
  executable call, and nested machine applications fail checking. A future
  language change may add a task only with its own semantic owner and concrete
  customer.

  Do not add a producer-executable path-byte commitment to review rows,
  closure commitments, conflicts, or admission while **Retire
  compiler-executable path-byte commitments that cannot identify execution**
  remains open in `OWNER_QUESTIONS.md`.

- [ ] **BUILD-OBSERVATION-EVIDENCE.** Extend the closed exact replay lanes for
  candidates that require receipted builds. The implemented grammar, versions,
  ceilings, and deliberately non-receipted neighbors live in
  `source/omega-rust/omega/build/omega-build-evaluation/BUILD_OBSERVATION_REPLAY.md`.

  The first two exact denied-operation rungs are complete: literal-mode
  Source-root `create` and operand-free Source-root `remove` retain their
  rooted operand and exact grant refusal, replay as compiler policy before
  generic Output handling, and survive canonical recovery. Broader refusal
  families remain below; add one only for a concrete receipted-build need,
  not to fill out an operation matrix.

  No additional replay operation is currently queued. The concrete
  `generated-table` package already closes its complete Source
  open/read/close, Output create/write/close, and generated-source handoff
  lifecycle through provider-free replay and canonical recovery. Add another
  operation or failed/denied outcome only with an otherwise admission-eligible
  package fixture that remains volatile solely because that exact replay is
  missing. Do not complete the raw filesystem operation matrix speculatively.
  Add peak-live accounts only where the compiler owns the complete allocation
  lifetime; do not duplicate per-operation bounds or present partial
  allocator/RSS participation as containment.
  Host CPU/RSS limits are deployment availability policy, not package evidence
  and not a precondition that turns review into authority. Projects that need
  stronger availability isolation run `omega` under their selected CI,
  container, VM, or job controls.

  A summary or observation digest alone is not a receipt.

  The Windows `find_first`/`find_next`/`find_close` companion remains ordered
  after **OPTIONAL-STDLIB-BUILD-PROTOCOL-AND-SEMANTIC-BINDINGS**. Its unrooted,
  working-directory-dependent pattern cannot become a portable receipt.
  Replace it with the root-aware Build facet before admission; do not add a
  same-path-only receipt or ignore the pattern during replay matching.

- [ ] **PROOF-AND-BOUNDARY-ADMISSION.** Add the authority-bearing later-
  discharge/open-obligation ledger and locally recheck every retained
  certificate required by an ordinary package claim. Package review already
  rejects contract-entailment stand-downs and retains accepted axioms, opaque
  claims, and bodyless accepted claims as explicit trust rows. Do not cite the
  standalone `psi-proof` ledger as production enforcement or invent a deferred-
  proof row before the compiler owns such a status.

- [ ] **FINAL-REALIZATION-EVIDENCE.** Require exact Terminal evidence only for
  claims about emitted native/external code, ABI/lowering-dependent guarantees,
  fixed native resources, or profiles requesting final-code replay. Keep
  ordinary checked capability/API evidence and opaque executable-supply rows in
  their distinct evidence classes; absence of Terminal evidence grants no
  Terminal claim.

  Terminal retains the exact settled join
  `{ authored operator-use coordinate, selected requirement/candidate } ->
  { selected realization machine, selected realization state }` as an ordinary
  scalar `Call`, without a global conformance scan. Immediately before either
  Terminal or NativeArtifact production, Omega rejoins that carrier to the
  complete still-owned selected ProviderPlan and rejects substitution by a
  second conforming checked machine. Named/fixed-token compiler canaries replay
  the canonical Terminal artifact through the later boundary consumer.
  NativeArtifact production now carries the first bounded attached-Unit scalar
  closure end to end: service-free fixed 8/16/32/64-bit integer callees,
  constants or prior call results as arguments, one durable Unit-frame home per
  result, exact per-function ABI, native call/relocation custody on all four
  ordinary targets, independent object byte replay, and installation format 46
  transport. Broader mixed scalar forms remain incremental engineering.
  D41 requires a distinct consuming-lowerer TCB lane for compiler-owned target
  builtins. Delete `CompilerIntrinsicSettlementEvidence` and its self-issued
  compact coordinates; they are not package-installation
  `ProviderExecutionEvidence`. Rejoin the exact Terminal requirement and
  selected structural intrinsic proposal to the receiving lowerer's target
  catalog, then bind emission through D32's physical child. Installed and
  foreign implementations retain their actual execution custody.

  Make the standalone Psi product sufficient for an independent realization
  consumer: selected provider-plan facts, external-binding requirements, and
  the requested target/profile proposal must be canonical product sections or
  an exact strongly bound companion, never hidden `CheckedCompilation` state.
  Add a reload canary that discards all frontend objects, decodes only the
  published Psi product plus an independently supplied `NativeRealizationRequest`,
  and either reproduces the native result or rejects an unavailable proposal.

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

  General target layout now consumes the exact selected opaque-to-carrier
  application and derives by-value size/alignment from the carrier while
  references remain representation-free and a direct unselected by-value
  demand rejects. Complete movement/lifecycle planning next under D44's
  inert-carrier rule. Do not publish a partial demand row from
  calling-convention shape or size/alignment alone. Add canaries proving that
  independently reviewed dependencies may retain different historical
  selections while one later source consumer selects its own application. The
  future `PackageInstance` composition canary must reject unequal commitments
  only at an actual by-value exchange.

## P3 — Recheckable evidence and accepted lock

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
  evidence means fresh graph admission. Unavailable old source produces a
  standalone-candidate review packet and audit recommendation; it neither
  proves an audit occurred nor erases a valid accepted baseline. No review-only
  capsule may be promoted by renaming it.

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

- [ ] **SOURCE-AND-PROVENANCE-TRIAGE.** Produce the bounded advisory-review
  request beside deterministic source/provenance triage. An optional
  organization-selected runner may invoke a reviewer and return the existing
  closed monotone recommendation. Package correctness, install/update
  availability, and deterministic recommendations must be identical when no
  runner is configured or reachable. Provider selection, credentials, network
  access, and invocation are operator tooling, not package-core gates. A
  supplied advisory result may only add an audit recommendation through root
  policy; it cannot suppress deterministic recommendations, resolve conflicts,
  admit evidence, attest review, or mutate project state.

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

- [ ] **PRIVATE-REMOTE-FIXTURES.** Run the exact pinned CathedralOS SSH/HTTPS
  mirror tests in credentialed infrastructure. The 2026-08-30 host-SSH run
  reached the pinned repositories but found the remote fixture contents stale
  against the current local language fixtures; synchronize and repin all
  changed mirrors, then rerun the complete lane. Unavailable credentials must
  remain an explicit ignored/blocked environment condition, never a fallback
  to a different transport or fabricated success.

## P7 — Cross-system package work

These tasks consume settled language and architecture decisions across package,
compiler, and runtime owners. A task that still needs an owner decision says so
explicitly.

- [ ] **OPAQUE-BY-VALUE-BOUNDARY-ABI — propagate the selected application.**
  - [x] carry the same application into general target layout;
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

- [ ] **D42-ALL-TARGET-CHECK — decide whether the convenience command exists.**
  **OWNER-BLOCKED — Retire or explicitly source all-target matrix
  enumeration** in `OWNER_QUESTIONS.md`. Flat unconditional dependency
  projection, canonical subject encoding, exact-target-only traversal, and
  rejection of retired conditional forms are complete. Do not invent target
  discovery or matrix evidence while the remaining command has no
  authoritative meaning for `all`.

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

  The ordinary package path now uses compiler-owned `BuildPath`, `BuildSource`,
  and `BuildOutput`; exact reachable facet calls activate the existing sponsor,
  and the live `generated-table -> generated-consumer` canary proves Source
  open/read/close, Output create/write/close, explicit handoff, and dependency
  injection without rerunning the producer. Finish the migration by deleting
  the explicit-`FilesystemHost` compatibility prelude, migrating its remaining
  compiler tests and any concrete build customers, and removing the std-owned
  `Path`/`Build.filesystem` route entirely. Do not duplicate the raw std
  filesystem matrix on the Build facets without a package-build customer.
  Build evaluation must admit no ordinary runtime boundary service merely
  because it is filesystem- or console-shaped.
  `FilesystemSponsor` remains the enforcement boundary for source/output
  roots, symlinks, limits, descriptors, and staging custody.

  Audit every non-test `SourceOrigin::Toolchain` consumer. Preserve it for core,
  intrinsics, and virtual compiler sources such as `<build-prelude>`; replace
  relocated std checks with ordinary `PackageKeyIdentity` provenance. Where
  target integration or dangerous-authority review genuinely needs compiler
  recognition, carry an accepted-closure-scoped binding to the exact nominal
  declaration and normalized schema fingerprint rather than granting a role to
  its whole package. Candidate review designations remain non-authoritative;
  accepted bindings come only from consumer policy.

- [ ] Complete generic and non-empty exact-application coverage for
  **BOUNDARY-OPERATOR-FAMILY-SELECTION**. Derive concrete static applications
  from checked uses, retain D29's normalized tagged type/const bindings, close
  artifact-qualified symbolic demands during final specialization, recheck the
  selected role-specific semantic realization, attach rows to production
  selected plans, and add compiler-to-update tests. Then implement D32's exact
  native physical children over the validated optimized projection, including
  per-occurrence parent bindings and missing/duplicate/stale/substituted/padded
  rejection. Exercise distinct and equivalent const values.
  Keep compatibility failure when a public family gains an uncovered
  coordinate. Universal generic coverage remains deliberately unimplemented
  under D28; package evidence must never substitute declaration order, display
  signatures, ordinals, authored assertions, bootstrap lowering, or
  reach-selected subsets.

- [ ] Consume **TOP-LEVEL-BOUNDARY-REQUIREMENTS** from `TASKS.md`: publish the
  explicit requirement declaration separately from every checked/external
  satisfier and selected provider. Retain visibility, exact operation/static
  telescope/signature/contract, authored selection custody, bounded reach,
  installed execution and era, and disclosed admissions. Neither equal reach,
  bodylessness, catalog presence, nor build policy may synthesize a requirement
  or satisfier edge. Package disclosure and payload-bearing external ABI-row
  extraction are complete for the current nongeneric lane; selected invocation
  must still replay the installed execution and era before this item closes.

## P8 — Final release gate

- [ ] **PACKAGE-MANAGER-RELEASE-AUDIT.** Before enabling mutation, rerun the
  complete package, package-evidence, package-compilation, resolver, compiler
  handoff, platform-native, fixture, recovery, and architecture suites. Define
  the exact expected-ignore allowlist and retired surfaces, require no physical
  std special-casing or unresolved canonical evidence rows, and verify a clean
  atomic failure path for every install/update stage.
