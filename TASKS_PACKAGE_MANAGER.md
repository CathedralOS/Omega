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

  Remaining projection work includes:

  - non-identical conformance-bound external refinements and the remaining
    selected provider-demand and installation-issuance joins;
  - **OWNER-BLOCKED — Bind lifetime arguments on exact machine requirement
    realizations** in `OWNER_QUESTIONS.md` before admitting lifetime-
    parameterized external trait realizations. The current machine edge retains
    no target-trait lifetime application, and package review must not infer one;
  - finish D29's artifact-qualified symbolic demand and final substitution,
    role-specific checked-realization replay, and D32's exact optimized-
    projection-to-physical-child join. Each physical child must bind its exact
    role-tagged `PhysicalChildParent` and surviving optimized operation
    occurrence; the complete child set must equal the derived surviving set.
    D29 parents reference reconstructible operator coverage; D41 parents retain
    and replay complete boundary-trait settlements. Lifetime, machine, and
    proposition operator applications remain fail-closed. D28 authorizes no
    universal generic coverage; only compiler-derived demand joined to an
    independently checked application-specific realization may authorize a
    row.

  Extend the earliest coherent compiler-owned representation that owns a
  missing fact. Do not reconstruct identity from diagnostics and do not add a
  nominal Chi stage merely to collect private compiler state.
  Do not create package-review work for forms the language rejects: proposition
  parameters are trait-only, proof-static evidence cannot eliminate into an
  executable call, and nested machine applications fail checking. A future
  language change may add a task only with its own semantic owner and concrete
  customer.

  D46 forbids producer-executable path-byte commitments in review rows,
  closure commitments, conflicts, locks, or admission. Same-process review
  compatibility uses the explicit semantic and evidence-encoding identities;
  it never substitutes the bytes readable through `current_exe()`.

- [ ] **FINAL-REALIZATION-EVIDENCE.** Require exact Terminal evidence only for
  claims about emitted native/external code, ABI/lowering-dependent guarantees,
  fixed native resources, or profiles requesting final-code replay. Keep
  ordinary checked capability/API evidence and opaque executable-supply rows in
  their distinct evidence classes; absence of Terminal evidence grants no
  Terminal claim.

  D41's first consuming-lowerer TCB lane is complete for Linux
  `exit_group(i32)`: the exact Terminal requirement and selected structural
  proposal rejoin the local target catalog, propagate as the role-tagged
  `CompilerBuiltin(LinuxExitGroupI32)` physical record, and mint no provider
  execution or installation receipt. Installed and foreign implementations
  retain their disjoint admitted execution custody. Extend the closed builtin
  sum only with another demanded local target mechanism. Convert planner
  classifications to `CompilerBuiltinExecution` with one exhaustive
  `match -> Option`, then finish D32's occurrence-specific D41 physical-child
  parent and replay rather than adding pre-lowering fingerprints.

  The first standalone-product lane now retains the selected provider-plan
  facts, external-binding requirements, target/profile, ProgramEntry, and
  compiler-builtin proposals as one exact companion to the canonical Terminal
  artifact. Its reload canary destroys frontend custody and realizes Linux ELF
  using only that retained product plus independently supplied proof admission
  and optimization inputs. Keep this carrier complete as additional native
  proposal classes land; do not regress to hidden `CheckedCompilation` state
  or replace its full structures with compact report fingerprints.

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

  Complete movement/lifecycle planning under D44's inert-carrier rule. Do not
  publish a partial demand row from
  calling-convention shape or size/alignment alone. Add canaries proving that
  independently reviewed dependencies may retain different historical
  selections while one later source consumer selects its own application. The
  future `PackageInstance` composition canary must reject unequal commitments
  only at an actual by-value exchange.

## P3 — Recheckable evidence and accepted lock

- [ ] **PROOF-AND-BOUNDARY-ADMISSION.** Complete the authority-bearing later-
  discharge/open-obligation result and locally recheck every retained
  certificate required by an ordinary package claim. The first real lane is
  complete in memory: a bodyless accepted claim rejoins its typed callable and
  canonical obligation row, remains explicitly `OpenRootAdmission`, and
  propagates through the exact dependency closure without producer decisions.
  Fresh admission now rederives that obligation closure and the empty-baseline
  conflict set together, requires a bijection between dependency-owned open
  claims and exact added `AcceptedClaim` conflicts, and replays a complete
  candidate-bound root policy before exposing the in-memory acceptance. A
  rejected, missing, stale, foreign, or row-substituted policy fails; a closure
  with no blockers gets no synthetic policy record. This still issues no
  certificate, accepted lock, package instance, or mutation authority.
  Add another result class only with a concrete compiler-owned obligation and
  certificate route or explicit open status. Do not persist this partial lane,
  cite the standalone `psi-proof` ledger as production enforcement, or invent
  a deferred-proof row before the compiler owns such a status.

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

- [ ] **LOCK-BASELINE-RECOVERY.** Persist and recover accepted baselines with
  strict canonical framing and immediate local reconstruction. Missing lock
  evidence means fresh graph admission. Unavailable old source produces a
  standalone-candidate review packet and audit recommendation; it neither
  proves an audit occurred nor erases a valid accepted baseline. No review-only
  capsule may be promoted by renaming it. An unsupported historical lock is
  retained opaquely for its matching old toolchain or separate audit tooling;
  current Omega does not migrate or grandfather its acceptance.

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
  occurred. The review-only primitive already emits exact added conflicts for
  fresh accepted claims, dangerous authority, and external executable supply
  against an explicit empty admission baseline; it does not synthesize old
  resolution or evidence. The in-memory fresh-admission gate now requires every
  blocking row to have an exact accepted root decision and binds open accepted
  claims to their own package rows. The remaining work is the accepted-lock
  reopen/revalidation and atomic install/update transaction, not another review
  receipt.

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

  The Windows `find_first`/`find_next`/`find_close` companion remains ordered
  after this migration. Its unrooted working-directory-dependent pattern
  cannot become a portable receipt; replace it with the root-aware Build facet
  before admission rather than adding a same-path-only replay rule.

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
  the role-tagged D29/D41 `PhysicalChildParent`, complete D41 settlement
  retention and replay, per-occurrence parent bindings, and
  missing/duplicate/stale/substituted/padded/role-swapped rejection. Exercise
  distinct and equivalent const values.
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
