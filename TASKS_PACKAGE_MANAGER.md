# Tasks: Package Manager

Status: remaining work only, 2026-08-29.

This file is the forward queue for the Cargo-like source/package service under
`omega`. Completed milestones live in Git history and in the subsystem notes;
they are deliberately not repeated here.

Governing documents:

- `wiki/design_briefs/package_manager_first_draft.md`
- `wiki/design_briefs/build_and_package_model.md`
- `wiki/language_guide/chapter_15_modules_imports_visibility.md`
- `wiki/language_guide/chapter_19_capabilities_effects_boundaries.md`
- `source/omega-rust/omega/packages/omega-package-manager/README.md`
- `source/omega-rust/omega/packages/omega-package-source/SOURCE_RESOLVER_SECURITY.md`
- `OWNER_QUESTIONS.md`

Do not wire mutating `omega install` or `omega update` until the P0 source
boundary, recheckable evidence, accepted lock, and transaction gates below are
closed. Compiler-issued package review remains non-admitting.

## P0 — Source resolver boundary

- [ ] **HARDEN-SOURCE-RESOLVER.** Finish the hostile-process boundary around
  local and Git resolution.

  Remaining work:

  - complete Linux metadata/read, direct-egress, and endpoint confinement.
    Landlock ABI v5 already constrains handled content/namespace mutation and
    path-based execution when fully available, but the complete write and
    executable rows remain unavailable: metadata operations such as mode/time
    changes are unmediated, and executable memfds or anonymous executable code
    require an additional mechanism such as seccomp. Package resolution rejects
    the resource-limit-only Linux fallback;
  - run the Linux Landlock write, inherited-descriptor, and exact-executable
    canaries on a native ABI-v5 worker; cross-compilation is not execution
    evidence;
  - provide the corresponding Windows filesystem, network, and executable
    confinement, plus the still-unavailable address-space, file-size,
    descriptor, and core-dump guarantees;
  - run the existing Windows Job Object process-count, per-process memory,
    aggregate-memory, and aggregate-CPU exhaustion pairs on a native Windows
    worker; cross-compilation is not execution evidence;
  - narrow macOS SSH discovery/fetch reads after OWNER Q5 settles explicit host-key,
    key, credential-provider, and credential-file custody;
  - make the existing broker transfer ceiling complete by denying direct helper
    egress on Linux and Windows; separately enforce whole-operation object-store,
    temporary-disk, descendant CPU/memory/process, and during-write quotas
    rather than only rejecting oversized retained state after the helper exits;
  - decide and implement the stronger isolation needed against hostile
    same-user cache/source mutation, executable replacement, loaded-image
    substitution, and hostile Unix descendants escaping their process group;
  - finish the existing locally reconstructed opaque strict receipt. The landed
    kernel exactly rejoins non-admitting resolution, native policy/completion,
    command input, endpoint, executable-path, transfer-accounting, source, and
    limit rows; calls `require_strict` for every command; and retains a closed
    rejection for missing, changed, unavailable, or the first unimplemented
    source requirement. There is deliberately no success issuer yet. Add real
    evidence carriers and reconstruction for transport-trust, credential-custody,
    whole-operation storage/resource, same-user mutation, and platform-native
    rows before any success receipt can issue;

  The detailed established floor and remaining platform gaps are maintained in
  `source/omega-rust/omega/packages/omega-package-source/SOURCE_RESOLVER_SECURITY.md`.
  Strict SSH trust and credential authority is
  design-blocked on OWNER Q5 (strict SSH custody); the other bullets are engineering work.

## P1 — Total package semantic identity

- [ ] **COMPLETE-CONFORMANCE-IDENTITY.** Retain complete public conformance
  applications, including the declaration-site target-trait lifetime mapping
  owned by **CONFORMANCE-TARGET-LIFETIME-APPLICATION** below. Unsupported
  generic, lifetime-bearing, private, or aliased forms must continue to fail
  closed rather than disappear from review.

## P2 — Total compiler admission projection

- [ ] **PACKAGE-ADMISSION-COMPILATION.** Make the compiler-owned ordinary
  package projection total for the supported language surface after successful
  checking. The canonical output must contain no arena handles, diagnostic
  strings as identity, or compiler-private IDs.

  Remaining projection work includes:

  - advanced call-bearing domain predicates, remaining computed/aggregate
    contract expressions beyond exact nominal-member projection, and
    structural witness arguments not retained by their owning typed or checked
    representation;
  - remaining semantic-role, operator, selected-provider grant, installation,
    permission-frontier, crash-refinement, and compiler-intrinsic ownership
    joins;
  - generic/exact-application boundary-provider family evidence under
    **BOUNDARY-OPERATOR-FAMILY-SELECTION**;
  - complete exact semantic-subject commitments, certificate closure, and
    reproducibility dispositions.

  Extend the earliest coherent compiler-owned representation that owns a
  missing fact. Do not reconstruct identity from diagnostics and do not add a
  nominal Chi stage merely to collect private compiler state.

- [ ] **BUILD-OBSERVATION-EVIDENCE.** Generalize the existing exact
  Source-input/empty-Output and repeated direct-child
  `create`/zero-or-more full sequential-or-positioned writes, exact successful
  seeks, successful length changes, descriptor-scoped permission changes, and
  successful syncs/`close` output replay lanes,
  including exact cursor-independent positioned offsets and exact ordered
  generated-source subsets, into a complete receipted build-operation and
  output grammar. Add replay for every remaining admitted service and staged-
  output shape, exact staged-output
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
  with exact selected or compiler-derived ABI and mechanism evidence whenever a
  runtime by-value use demands representation closure. Retain the exact opaque
  declaration, named representation conformance or compiler-owned target-
  semantics application, closed shape graph, physical movement/finalization
  plan, target/representation version, and evidence origin. Checked carrier
  derivation is recheckable evidence; foreign representation supply remains a
  disclosed admission. Claim-free opaque data stays review-visible without
  fabricating a proposition, minting authority, or service reach claim.

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
  authority. For an authority surface declared by an ordinary package, bind the
  closed risk class to the exact accepted declaration identity and normalized
  schema, never to a package-wide role. Classification is consumer policy and
  review metadata; it does not grant the service or its provider authority.

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

## P7 — Cross-system package work

These tasks consume settled language and architecture decisions across package,
compiler, and runtime owners. A task that still needs an owner decision says so
explicitly.

- [ ] **OPAQUE-BY-VALUE-BOUNDARY-ABI.** Implement lazy representation demand for
  runtime-relevant opaque boundary data. Add compiler-owned
  `OpaqueRepresentation<Opaque>` with ordinary type-parameter spelling. A named
  conformance such as `PicAckCarrier satisfies
  OpaqueRepresentation<InterruptAcknowledgement>` declares an inert candidate;
  typed `Build::select_representation<Opaque, Conformance>()` selects only that
  already-authored relationship. The compiler derives the carrier's closed
  representation rather than accepting source-authored sizes, alignments, ABI
  classes, or numeric IDs.

  Require one exact target-closed representation application before evaluating
  any runtime by-value calling plan. References do not demand the pointee's
  representation, and proof-erased boundary data demands none. Permit compiler-
  sealed families such as `Ptr<T>` to resolve from target semantics without a
  package candidate. Keep representation source, minting route, and domain
  authority as separate evidence lanes. The opaque declaration owns semantic
  multiplicity and terminal discharge; the carrier contributes only physical
  movement and storage finalization, which must compose with rather than replace
  that discharge.

  Rejoin every producer and consumer to the same application. Retain its exact
  identity in type layout, boundary signature, calling-plan application,
  package review, artifact compatibility, and replacement-facing contracts.
  A by-value exported descriptor is a mandatory static compatibility row:
  independently replaceable providers must preserve it. A stable handle keeps
  its own descriptor fixed while provider backing may vary and outstanding
  non-copy handles pin their era; an unstable descriptor expands the replacement
  cohort to consumers or rejects independent replacement. Missing, duplicate,
  conflicting, stale, ambient, runtime-installation-selected, and policy-
  invented representations reject. Add pass/fail coverage for inline interrupt
  carriers, compiler-owned pointers, reference-only `EfiSystemTable`, proof-only
  `Real`, wrong opaque subject, lookalike representation traits, provider drift,
  descriptor replay drift, and illegal carrier cleanup/multiplicity changes.

- [ ] **STATIC-TARGET-CONDITIONED-DEPENDENCIES — project ordinary target transitions without executing `build.omg`.**
  Extend the syntax projector to derive a target-independent
  `ProjectedDependencies { common, by_profile }` map from the finite build
  state graph. Follow unconditional transitions and exact
  `transition builder.target` arms; intersect nested exact constraints, merge
  shared states, handle cycles by fixpoint, and project each authored call once.
  A dependency occurrence must have an authorized path and no path tainted by a
  wildcard target arm or transition on another runtime subject. Reject tainted,
  mixed-path, and unreachable occurrences with the dependency span plus the
  transition/arm provenance and a directed repair. Do not add `depend_when`,
  `depend_as_when`, condition strings, or a second dependency grammar.

  Validate exact profile keys against the trusted toolchain target catalog at
  projection time. Retain the condition-schema version and identities of only
  the profiles actually referenced. Alias uniqueness is checked within
  `common + by_profile[P]`: mutually exclusive profile columns may reuse an
  alias, while a common alias conflicts in every column. Make package review
  distinguish the complete projected map for that one fetched package from an
  unresolved transitive graph. Extend the one workspace lock with independently
  populated per-profile closure/review sections; locked resolution of an absent
  column rejects without network access, while an explicit operation may
  populate all columns. Add canaries for unconditional factoring, nested exact
  intersections, shared states, cycles, wildcard and runtime-subject taint,
  mixed paths, unreachable calls, alias reuse/conflict, catalog growth, stale
  profile identity, and missing locked columns.

- [ ] **APPLICATION-ROOT-ROLE-EVIDENCE — retain the admitted root role after resolution.**
  Source projection no longer coerces `ApplicationDeclaration` into a package.
  Source custody and reconciliation retain the exact root role, reject an
  application behind every dependency edge, and bind the role into canonical
  source-closure v4 and review-baseline v4 recovery. Explicit project-root
  entry points cover external-local, Git, named Git member, and workspace
  sources. Compiler handoff retains the same `BuildDeclarationKind` in
  `PackageCompilationInputs`, its source-path-free dependency closure, and
  ordinary obligation-ledger v2 recovery. Source-consumption v3 and production
  manifest v2 identities bind it; package-only callers use the explicit
  `new_package` constructor.

  Remaining work:

  - retain `{ PackageKey, BuildDeclarationKind }` through accepted lock rows,
    command diagnostics, and audit output;
  - compare root-role changes directionally and add package/application replay,
    tampering, and role-change fixtures at each downstream boundary.

- [ ] **PACKAGE-NATIVE-GENERATED-SOURCE-TRANSACTION.**
  Route native-image production through the sponsored package transaction
  without rerunning `build.omg` or reopening discovery. Lower the exact frozen
  checked program after generated-source handoff, retain the unpublished native
  artifact as an exact subject, reconstruct every source/build/generated/native
  commitment, and publish only after complete accepted comparison. Consume the
  retained application-root `PackageKey` and role above; exact requested-target
  identity comes from **IMMUTABLE-TARGET-ACTIVATION-AND-REACH-CLOSURE** in
  `TASKS.md`.

- [ ] **SCOPED-BUILD-ROOT-RETIREMENT — enforce one canonical free project entry.**
  Delete the `has_scoped_build` early return that bypasses selected-role
  validation, remove scoped-name acceptance from compiler build-machine
  selection, and make package and standalone readers require the same exact
  free `machine build(builder: &mut Build)` root. A scoped `Owner::build` in
  ordinary source remains an ordinary machine; in a selected `build.omg` it
  rejects with a directed migration diagnostic. Remove the repository exception
  census and migrate compiler tests that positively select scoped roots.

  Convert all five corpus exceptions to free roots and give every one an
  explicit application role. Move the two provider-selection passes into the
  root or an ordinary `configure_*(&mut Build)` helper. Recast
  `build_effects_undeclared` through a compiler-owned Build facet while omitting
  the corresponding root build-effect ceiling; keep the rowless and lookalike-
  service failures on their exact declared reach identities without receiver
  smuggling until the std-service gate is retired.

  Pin helper composition with a positive and negative pair. The free root may
  lend `&mut Build` to an ordinary helper, but the helper's complete transitive
  reach, invocation, suspension, blocking, termination, authority, and build
  observations compose into the root. A source-reading or output-writing helper
  called from a root without the corresponding Build-facet effect ceiling must
  reject. Project-role, workspace, and dependency declarations remain direct
  statically projected root statements and reject inside helpers.

- [ ] **CONFORMANCE-TARGET-LIFETIME-APPLICATION — complete the conformance
  header's trait application.** Add the ordered target-trait lifetime arguments
  beside its existing type arguments in syntax, symbol-resolved, typed, checked,
  snapshot, and semantic-identity carriers. A conformance declaration supplies
  every target lifetime explicitly; require exact arity and resolve each source
  name only to an in-scope conformance lifetime binder. Retain alpha-normalized
  declaration-order ordinals through direct and inherited requirement
  substitution, specialization, public conformance evidence, canonical encoding,
  recovery, and compatibility comparison. Package review consumes checked
  ordinals and never reconstructs them from spelling, subject shape, or expected
  trait arguments.

  Preserve the existing application-site rule independently: omitted generic-
  conformance lifetimes are accepted only when ordinary call borrow constraints
  yield one unique complete mapping, and the resolved mapping enters semantic
  identity before review. Add positive explicit declaration and concrete-
  substitution canaries; exact-arity and undeclared-binder failures; inherited-
  requirement substitution; binder-rename stability; changed-ordinal identity
  drift; and a zero-candidate application canary distinct from the existing
  unique and conflicting-constraint cases. Exact ordinal equality remains both
  identity and selection until Omega gains another lifetime term or lifetime
  subtyping.

- [ ] **OPTIONAL-STDLIB-BUILD-PROTOCOL-AND-SEMANTIC-BINDINGS.** Finish the
  ordinary-package std migration without recreating a privileged `std` role.
  Only core and genuinely compiler-injected vocabulary remain toolchain-owned;
  std may be replaced, split, or absent. Removing its graph edge must reject
  every std import or provider selection, and no name, alias, path, repository,
  filename, or same-spelled declaration may restore it.

  Remove the build protocol's dependency on std: delete
  `Build.filesystem: FilesystemHost`, replace the std-owned `Path` in the
  `BuildSource`/`BuildOutput` surface with a compiler-owned relative-path
  carrier or direct rooted operations, move sponsored reads and writes onto
  those existing facets, and add an explicit compiler-owned logging facet.
  Build evaluation must admit no ordinary runtime boundary service merely
  because it is filesystem- or console-shaped. Retain exact build-effect and
  observation rows, while `FilesystemSponsor` remains the enforcement boundary
  for source/output roots, symlinks, limits, descriptors, and staging custody.

  Audit every non-test `SourceOrigin::Toolchain` consumer. Preserve it for core,
  intrinsics, and virtual compiler sources such as `<build-prelude>`; replace
  relocated std checks with ordinary `PackageKeyIdentity` provenance. Where
  target integration or dangerous-authority review genuinely needs compiler
  recognition, carry an accepted-closure-scoped binding to the exact nominal
  declaration and normalized schema fingerprint rather than granting a role to
  its whole package. Candidate review designations remain non-authoritative;
  accepted bindings come only from consumer policy. Clear the
  `generated-table -> generated-consumer` canary through the sponsored Build
  facets, not through a std authority exception.

- [ ] Complete generic/exact-application coverage for
  **BOUNDARY-OPERATOR-FAMILY-SELECTION**. Package review now retains complete
  declaration-family coordinates, exact provider and target, selection
  authority, and the canonical coordinate-to-plan mapping. Extend that closed
  carrier when exact applications become admissible, and keep compatibility
  failure when a public family gains an uncovered coordinate. Package evidence
  must never use declaration order, display signatures, ordinals, or
  reach-selected subsets.

- [ ] Consume **TOP-LEVEL-BOUNDARY-REQUIREMENTS** from `TASKS.md`: publish the
  explicit requirement declaration separately from every checked/external
  satisfier and selected provider. Retain visibility, exact operation/static
  telescope/signature/contract, authored selection custody, bounded reach,
  installed execution and era, and disclosed admissions. Neither equal reach,
  bodylessness, catalog presence, nor build policy may synthesize a requirement
  or satisfier edge.

- [ ] **BLOCKED — OWNER Q5: STRICT-SSH-CUSTODY.** Settle host-key, key,
  credential-provider, and credential-file authority before narrowing the
  remaining SSH read surface or treating SSH resolution as strict evidence.

## P8 — Final release gate

- [ ] **PACKAGE-MANAGER-RELEASE-AUDIT.** Before enabling mutation, rerun the
  complete package, package-review, package-compilation, resolver, compiler
  handoff, platform-native, fixture, recovery, and architecture suites. Require
  zero unexpected ignores, no legacy manifest/receipt/plan surface, no physical
  std special-casing, no unresolved canonical evidence rows, and a clean atomic
  failure path for every install/update stage.
