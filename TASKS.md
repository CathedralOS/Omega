# Tasks

This file is the current cross-project execution board, not a changelog.
Completed work belongs in Git history and the durable architecture/design pages.
Detailed bootstrap, optimizer, and package-manager work lives only in
[`TASKS_BOOTSTRAP.md`](TASKS_BOOTSTRAP.md),
[`TASKS_OPTIMIZER.md`](TASKS_OPTIMIZER.md), and
[`TASKS_PACKAGE_MANAGER.md`](TASKS_PACKAGE_MANAGER.md).

A task remains here only when it names:

- unfinished work;
- its owning code/design area;
- a real blocker, if one exists; and
- a concrete acceptance condition.

Remove a task when its acceptance condition passes. During an active change,
retain only the context needed to resume it. Do not append landed substeps,
version-bump history, test counts, or release notes here. If a task grows beyond
roughly three short paragraphs, move the design detail to its owning document
and leave a link plus the next executable step.

Before starting work, fetch `main`, inspect the newest commits in that lane, and
avoid overlapping an active change. Commit and push coherent milestones.
Engineering difficulty is not a design blocker. Owner decisions belong in
`OWNER_QUESTIONS.md`. Research without a current customer does not belong on
the execution board; recover it from its design document or Git history when a
real customer appears. Do not mirror owner-question or customer-gated indexes
here.

## Ownership firewall

Psi operates on Omega source and owns parsing plus all target-neutral semantics
through Terminal Psi. Omega consumes Terminal Psi and owns provider selection,
optimization, target realization, native emission, and general execution
machinery. Target backends own unavoidable ISA, ABI, object-format, and
relocation encoding. Cathedral owns OS data structures, policies, protocols,
and lifecycle.

Compiler guarantees are established by checking and artifact verification;
they require neither an accepted package lock nor a proof-bearing
`PackageInstance`. Unfinished native realization does not block source package
installation. Unsupported compiler forms reject at their owning stage.

If Cathedral cannot express a subsystem, identify the missing general Omega
primitive or mark the slice blocked. Do not implement page tables, descriptor
tables, schedulers, process tables, timer queues, or drivers as compiler-owned
Rust models. Compiler validation and code generation may consume general plans;
they must not acquire customer-shaped semantic types or lifecycle protocols.

## Immediate product closure

These are the next product-level priorities for the maintained Rust
implementation. They take precedence over adding another evidence carrier that
has no exercising program. The finite definition of Rust-product completion is
the [Rust Compiler Completion Contract](wiki/releases/rust_compiler_completion_contract.md).

- **OMEGA-PRODUCT-COMPILER-SOURCE.** Establish the production compiler as two
  sibling Omega packages: target-neutral phases under `source/psi/` and the
  Terminal-Psi-consuming product under `source/omega/`, with hosted entrypoints
  at `source/omega/{build.omg,main.omg}`. The maintained Rust compiler is the
  differential implementation, not source for this task. Work backward from
  complete Omega behavior in small, live vertical slices; do not create a
  bootstrap-private dialect, file allowlist, or parallel source-to-native path.

  Acceptance: the exact Omega source closure implements the complete language
  and production pipeline, passes the shared product suite, and publishes a
  deterministic manifest of every transitive compiler/build input. Bootstrap
  construction of that closure belongs in `TASKS_BOOTSTRAP.md`.

`omega-rust/` remains the production implementation until that contract
closes. It may remain afterward as a differential implementation while it finds
real bugs, but Rust agreement is not bootstrap authority and Rust-specific
machinery must not migrate into the Omega-written compiler source.

## P1 - Authority, roots, and entry

Owners include
`wiki/design_briefs/authority_values_and_boundary_evidence.md` and
`wiki/design_briefs/canonical_ir_fuel_and_resource_provisioning.md`.

- **ENTRY-CONTENT-ROOTS.** Connect the generated target entry stub to the exact
  selected semantic continuation, consume the activation loan, and retain
  generated-bridge evidence without inventing roots. Migrate deployable
  fixtures to authored target-owned `ProgramEntry`; targetless checks select
  none. Acceptance is native execution from an authored entry with exact
  symbol/text/continuation replay and mutation failures for redirected or
  duplicated identities.

- **UEFI-PHYSICAL-SEMANTIC-ENTRY.** Finish the two-surface UEFI bridge: the
  target-package physical firmware entry remains distinct from the semantic
  program continuation. Emit and validate the adapter, exact calling plan,
  stack/custody transfer, and return behavior. Application lookalikes and
  cross-target substitutions must reject.

- **UEFI-OS-HANDOFF.** Implement the nonreturning custody transfer from Boot
  Services to the selected OS entry. The bounded memory-map/key retry loop must
  return all custody on stale-key failure and consume boot-scoped services only
  on success. Acceptance includes stale-key, exhaustion, lost-custody,
  post-exit provider-use, and successful handoff canaries.

- **CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS.** Carry one real
  content-bearing program through checked source, Terminal Psi, provider
  selection, and native realization. Introductions and exits must bind exact
  subject, geometry, lineage, route, and installed occurrence; reshuffles may
  preserve identity, while partitions require authored proof. Acceptance:
  every surviving content claim traces to a reconstructed introduction or
  admitted provider issuance and every residual is accounted for.

- **INSTALLED-PROGRAM-LOCAL-ROOT-INTRODUCTION.** Derive enumerable program-local
  content roots from exact installed parameter positions, capacity, and epoch.
  Ordinary results with no parent lineage cannot mint roots. Acceptance:
  aggregate capacity is reconstructed for one artifact instance and lifecycle
  epoch, with no ambient provision or row-equality authority.

- **BOUNDARY-ISSUANCE.** After conservation closes, derive provider issuance
  geometry from exact invocation parameters, entry places, and results. Keep
  ownership, aliasing, issuance, custody, and partition succession distinct;
  providers may attest custody but not computable interval arithmetic.

## P2 - Materialization and placed access

- **PLAN-LAID-VIEWS.** Finish checked and native placement for plan-laid views
  without turning a physical address into semantic ownership. Layout identity,
  backing, range, access, and lifetime must rejoin at every use. Acceptance:
  valid views survive codec/native replay and stale plan, range, access, or
  backing substitutions reject.

- **ACCESS-PLAN-AND-PLACED.** Finish the public `AccessPlan` / `Placed<P, T>`
  model as an explicit relation among semantic value, target layout, backing,
  and placement. Do not infer authorization from equal offsets or compiler
  custody. Acceptance: source can express and consume one useful placed value
  through target lowering while arbitrary construction and cross-plan reuse
  remain impossible.

- **SYMBOLIC-MATERIALIZATION.** Complete symbolic field/index materialization
  and its target-dependent realization. Preserve exact paths and bounds until
  assignment; physical lowering may choose locations but not change semantic
  access. Recursive build-time projection/replay is shared across the currently
  admitted exact record depths through 23; extend that recursive owner rather
  than adding another copied depth implementation. Nested sum arrays, direct-
  sum coexistence, recursive shapes, and target-dependent placement remain
  fenced until their general rules land. Acceptance includes nested field/index
  canaries on both Linux ISAs.

## P3 - Terminal Psi, PCC, and observation

- **PSIIR.** Extend Terminal Psi only in complete vertical slices through
  canonical encoding, independent reconstruction, verification,
  interpretation, resource analysis, native lowering, artifact custody, and
  installation. The detailed vocabulary lives in
  `wiki/architecture/pipeline/terminal_psi.md`; this task records no operation
  ledger. Acceptance: source and producer state can be discarded before an
  independent verifier reconstructs every obligation and executes or lowers
  the same artifact.

  Native/external execution, ABI, fixed native resource, and final-code replay
  claims additionally require exact final-realization evidence. Preserve
  complete standalone products without hidden `CheckedCompilation` state;
  checked API/capability results and opaque executable supply cannot establish
  those claims. Physical optimization replay belongs to
  `TRANSLATION-VALIDATION` in `TASKS_OPTIMIZER.md`.

- **CRASH-CONTRACT.** Complete invocation-specific crash obligations through
  nested structural paths, calls, cycles, and imported effects. Crash is an
  explicit observable outcome with a semantic cause; it is never represented
  as an ordinary return, missing cleanup, or backend trap inferred after the
  fact. Acceptance: safe calls discharge every route and mutations to guards,
  substitutions, or sites reject.

- **PROOF-CERTIFICATION-BRIDGE.** Emit kernel-checkable certificates from
  source automation. Recursive certificates own one SCC and cite ranking and
  well-foundedness evidence once; normalization names exact laws and preserves
  transitive trust. Acceptance: changing an edge decrease, premise, law, or
  component identity rejects or changes the trust closure. For separately
  compiled dependencies, reconstruct the exact obligations and recheck retained
  certificates locally; propagate unresolved assumptions with their original
  owner. Missing or stale evidence cannot silently discharge an obligation or
  inherit a producer's admission decision.

- **SUBJECT-QUALIFIED-ARTIFACT-PROOFS.** Bind every proof to an exact semantic
  subject and observation profile through ledgers, artifact seals, deployment,
  replay, and reports. Producers may not choose the verifier's root subject.
  Acceptance: a proof or commitment valid for one source/model/profile cannot
  be replayed in another role even when compact coordinates coincide.

- **PCC-CANONICAL-SEMANTIC-LEDGER.** Replace trusted Rust fusion of artifact
  traversal and proof search with a small total canonical-ledger generator plus
  an untrusted certificate producer. The verifier reconstructs goals and only
  checks the supplied route. Bootstrap discharge remains open under
  `BETA-DERIVATION-CHECKER` in `TASKS_BOOTSTRAP.md`; no current artifact may
  claim rooted-checker acceptance.

- **IRFUEL.** Keep fuel as analysis/evaluator evidence, never inserted runtime
  semantics. Extend installed-code correspondence from the bounded ranked
  countdown to ordinary admitted loops. Failure to derive a bound reports
  `Unknown` or `NoFiniteGuarantee`; it does not alter execution.

- **PROOF-RELEVANCE-MIGRATION.** Finish `[erased]` noninterference and
  erased-stripped layout across remaining carriers. Erased terms remain in
  semantic/proof identity but contribute no runtime storage, tags, ABI
  transfer, or execution. Runtime use and any layout-dependent erasure reject.

- **EFFECTFUL-TYPED-COMPUTATION.** Specify the value/computation judgments that
  connect effectful machines to the future typed proof calculus. This is
  semantic design work, not a prerequisite for extending unrelated Terminal
  operations.

## P4 - ABI, borrowing, and callbacks

- **NORMALIZED-ABI-LOWERING.** Finish target-independent signature
  normalization and target-owned calling/layout realization for aggregates,
  dynamic values, callbacks, and foreign boundaries. Acceptance: the ABI is
  independently reconstructible and no target placement leaks back into
  Terminal Psi.

- **OPAQUE-BY-VALUE-BOUNDARY-ABI.** Complete D26 representation agreement at
  independently compiled by-value exchanges. Rejoin consumer demands to exact
  producer opaque/conformance/carrier declarations and immutable source;
  enforce strong selected-application equality at actual exchanges. Finish
  physical movement and lifecycle planning, including D44 transitive
  inert-carrier proof and multiplicity checks. Equal size/alignment or compact
  fingerprints cannot establish agreement.

  Carry the application through native artifacts, replacement compatibility,
  stable-handle eras, and independently replaceable provider contracts.
  Acceptance: independently compiled producer/consumer and historical-selection
  canaries cover sealed `Ptr<T>` target semantics, proof-only `Real`,
  `EfiSystemTable`, provider/replay drift, cleanup, and multiplicity; incompatible
  by-value exchanges and replacements reject before execution.

- **WRITE-ONLY-BORROW.** Finish `&write T` through projected aggregates,
  calls, returns, dynamic dispatch, cleanup, and native lowering. It permits
  initialization without observation and must remain distinct from shared and
  mutable borrow. Acceptance includes read rejection, exact write coverage,
  unwind/return behavior, and both Linux targets.

  General native structural mutation and caller-visible writeback are
  OWNER-BLOCKED on the native structural-parameter identity decision in
  `OWNER_QUESTIONS.md`. Current projected-store byte and installation tests
  do not establish mutation of the caller's referent after return. Checked
  semantics, independent Terminal replay, and interpreter execution can proceed.

- **BORROW-PROOF-CONVERGENCE.** Make ordinary borrow checking proof-producing
  without allowing propositions to create or amplify authority. Normalize
  symbolic half-open ranges, then admit explicit compatibility theorems over
  already-existing places and occurrences. Acceptance: proof evidence can
  establish disjointness/containment but cannot extend lifetime, duplicate a
  loan, or replace ownership accounting.

- **CALLBACK-PARAMETER-REQUIREMENT.** Implement the nominal
  `where machine Selected satisfies Trait::requirement` binder and retain its
  exact requirement, conformance, envelope refinement, call site, and target
  entry recipe. Structural coincidence and overloaded/implicit selection
  reject.

- **CALLBACK-PRIVATE-MATERIALIZATION.** Add target-owned private callback slots
  selected through exact conformances and validated layout paths. Private
  slots must be absent from source-visible schema and inaccessible as ordinary
  fields or addresses. Acceptance: one outbound registrar closes without a raw
  code pointer or duplicated placement authority.

- **REGISTERED-CALLBACK-LIFETIME.** Model successful registration as a linear
  external root and unregister as the operation that ends it before releasing
  code/component leases. Capacity bounds live registrations, not emitted
  thunks. Acceptance covers rejection, retry, replacement, cleanup, and an
  actual Windows callback after the generic path closes.

- **FOREIGN-RETAINED-ARGUMENT-BACKING.** Generalize retained outbound arguments
  beyond callbacks with explicit call-scoped, lifetime-borrowed, moved, and
  snapshot dispositions. Every retained pointer needs exact stable backing,
  range, access, lifetime, and revision provenance; unknown or mutable ambient
  backing rejects.

## P5 - Cathedral over general Omega primitives

- **BUMP-ALLOCATOR-CANARY.** Build a package-level allocator over one qualified
  `Extent`, supporting two coexisting allocations, exact cleanup/recomposition,
  and reset only after full return. Use it to discover the real `Vec<T>`
  contract; do not add allocator semantics to the compiler.

- **ADDRESS-TRANSLATION-CANARY.** Continue Cathedral's page-table hierarchy,
  backing, policy, installation, and teardown in Omega source. Existing numeric
  page-walk validation grants no mapping authority. Acceptance: QEMU installs
  and tears down Cathedral-owned mappings with explicit Extent and TLB custody.

- **EXCEPTION-ROOTS-AND-TIMER.** Materialize all fatal exception entries,
  dedicated critical stacks, IDT installation, and a minimal timer root whose
  hard handler only acknowledges, records, and wakes ordinary work. Acceptance:
  QEMU reports timer ticks over owned output and halts between ticks.

- **BOUNDED-INSTALLATION-REACH-ROWS.** Finish unresolved-requirement fences for
  component contracts and the final carrier-owned invocation route. Concrete
  reach and conservative bounds remain separate; selected provider execution
  and token era, not row equality, authorize invocation.

## Parallel language and compiler lanes

- **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW.** Replace filename/trait heuristics with
  service permissions versus exact exercised terminal mechanisms. Classification
  is demand-complete over admitted provider leaves and fail-closed for unknown
  mechanisms. Argument-sensitive narrowing requires evidence that the lowering
  pins the relevant arguments. Acceptance: exercised classes must be a subset
  of the service's permitted classes under the receiving lowerer's versioned
  target policy. Exact selected-closure containment is live and the five legacy
  hardware filename/trait classifications are retired. Q4 blocks only the 14
  unresolved portable filesystem dispositions and eventual retirement of the
  transitional broad `Filesystem` review summary.

- **R5.** Finish exact inferred may-write summaries and relational candidates
  for unresolved receivers, boundary-result origins,
  moved and call-result reference-bearing aggregate origins, computed reference
  arguments outside proven helper-result relations, and other unsupported
  expression shapes.
  Prefer shared fixpoint and alias reasoning over syntax-shape exceptions.
  Acceptance: all supported finite source shapes converge without widening
  permissions, and unsupported recursion fails explicitly.

- **TPR6.** Finish subject-bearing progress-premise normalization through
  exported bodies, provider plans, recursive calls, and artifact evidence.
  Private ranking witnesses stay outside public identity. Acceptance: every
  used premise is reconstructed for the exact subject and no qualification or
  similarly shaped row mints one implicitly.

- **CML4.** Complete `EdgeCleanupPlan` after outgoing materialization and
  transfer commitment, including structural sums, nested projections, cycles,
  calls, and partial initialization. Cleanup follows reverse establishment and
  exact residual custody; trap/abort edges clean nothing. Acceptance: no affine
  occurrence disappears, duplicates, or is cleaned after transfer.

- **CLEANUP-HOOK-SELECTION-AND-ERASED-OWNERSHIP.** Finish ordinary generic
  `drop<T>` and runtime cleanup invocation after exact owner-attached hook
  selection. Erased fields remain semantically present but never produce
  runtime cleanup. Acceptance: every path invokes the exact selected hook once
  or proves the value transferred/consumed.

- **EXTERNAL-ENTRY-STACK-EPOCHS.** Finish exact enter/body/exit stack epochs,
  context-specific provider dispositions, finite nesting, and installed-root
  binding. Acceptance: WCSU, stack leases, artifact entry, and runtime context
  independently rejoin; unresolved or cross-context dispositions reject.

- **TR3-TR8.** Finish whole-call-graph worst-case stack derivation, exact
  `StackPlan`, nonmoving `StackLease`, suspension/cancellation preservation,
  transactional arguments, park/resume lowering, and the suspension-safe loan
  subset. Acceptance: stack/control custody is never compiler-owned or lost
  across a suspension edge.

- **BLOCKEXEC.** Implement a package-level blocking executor with bounded
  queues, moved custody, linear completion claims, suspension, and provider
  selection. Hung-worker recovery requiring termination must use process
  isolation.

- **SELECTED-WITNESS-EVIDENCE.** Finish executable proof-output calls beyond
  the unconditional Unit/scalar lane, preserving selected proposition,
  producer, optional local term, and runtime-call linkage. Acceptance: omitted,
  reordered, substituted, or unlinked witnesses reject without turning proof
  terms into runtime values.

- **TRAIT-NAMED-WITNESS-CONTRACTS.** Carry named proof inputs/outputs through
  trait requirements, conformances, calls, Terminal Psi, and independent
  verification. Names are public proof API only where declared; satisfier-local
  aliases remain local.

- **QUOTIENT-THEOREM-LIFT.** Admit explicit representative operation,
  congruence theorem, and optional precondition transport for quotient-owned
  operations. No structural or effectful observer crosses the quotient unless
  its law is explicit and checked. Custody-bearing quotients remain fenced.

- **EVALUATED-FOREIGN-BINDINGS.** Replace string-backed import bootstrap with
  typed compile-time locator values for PE, versioned ELF, and Darwin/Mach-O.
  Carry normalized locator, evaluated plan, target applicability, and producer
  custody through provider selection and native emission. Raw foreign bytes are
  data, never Omega symbol names or ambient lookup authority.

  Extend D41 normalized-import evidence from fixed-width scalar calls to a
  source-rooted flat-record argument, then ranked control and port-bearing
  artifacts. Acceptance: independent native replay preserves the exact
  survivor/physical-child bijection and rejects missing, duplicate, substituted,
  or role-swapped children. External realization claims require independently
  admitted concrete authority.

- **FLOAT-PROVIDERS.** Complete runtime Boolean/machine operations for exact
  `FloatMeaning`, kernel discharge, and remaining artifact-aware proof sources.
  Keep IEEE runtime comparison distinct from mathematical meaning equality;
  NaN payloads erase only in the meaning projection and signed zeros remain
  distinct there.

- **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY.** Materialize dynamic trait
  descriptors for pass-through, rebound, and escaping borrows from exact
  selected conformances. Calls may direct-devirtualize only when exact
  selection is proven; bodyless requirements and ambiguous carrier matches do
  not license `dyn`.

- **TARGET-SEMANTIC-APPLICATIONS.** Complete typed target observations,
  hermetic const evaluation, and D29 selected realization coverage. Finish
  artifact-qualified symbolic substitution for separately compiled generics;
  recheck the reachable specialization's actual capability reach, proof
  obligations, target facts, and selected realization after closing every
  argument. Boundary-operator empty telescopes remain distinct from
  boundary-trait calls with no telescope. Acceptance: cross-artifact canaries
  preserve actual reach and transitive open obligations, reject stale or
  substituted applications, and grant no coverage to unresolved arguments.
  D32 physical-child binding belongs to `TRANSLATION-VALIDATION` in
  `TASKS_OPTIMIZER.md`.

- **BOUNDARY-OPERATOR-FAMILY-SELECTION.** Extend build selection from exact
  boundary traits to exact package-qualified boundary-operator families.
  Selection is atomic over every overload coordinate and retains target plus
  generic/exact-application coverage. Partial, duplicate, stale, substituted,
  or padded family rows reject; equality of provider assertions is never
  realization coverage.

- **TOP-LEVEL-BOUNDARY-REQUIREMENTS.** Finish explicit public boundary
  requirement declarations, external satisfiers, provider selection, and
  installed execution/era replay. Remove transitional undifferentiated
  bodyless-machine modes once their source migrations close.

- **BUILD-ADMISSION-CHECKPOINT.** Execute an admitted build machine against one
  coherent frontend/source/authority snapshot and append generated source in a
  later resolution stratum. Authored source may not resolve forward into output
  generated by its own build. Acceptance includes replay after serialization
  and drift rejection for the full activation.

  Finish compiler-owned publication of the retained native product built with
  generated source. Bind the exact application root, authored declaration role,
  requested target, and source/build/generated/native inputs; validate final
  realization before publishing. Acceptance: serialized replay reproduces the
  product, and source, role, target, or artifact drift prevents publication.

- **OPTIONAL-STDLIB-SEMANTIC-BINDINGS.** Finish the compiler/library migration
  to explicit ordinary std dependency edges. Std may be replaced, split, or
  absent; only core and compiler-injected vocabulary remain toolchain-owned.
  Migrate package-aware fixtures, keep freestanding UEFI roots dependency-free,
  and retain standalone compatibility only until fixtures acquire package
  roots. Replace std/alloc `Toolchain` classification when compiler consumers
  have exact source-byte catalog entries or explicit semantic bindings.

  Complete composed-Unit plans for trait-default, float, wire, arithmetic-helper,
  guarded-call, and looping-cast canaries and the target-correct non-Linux
  Console catalog entry. Structural writeback shares the blocker recorded in
  `WRITE-ONLY-BORROW`. Feed consumer-scoped Console, Filesystem, and UEFI
  bindings through normal package-aware compilation. Acceptance: removing a
  dependency rejects its imports/provider selections; name, alias, path, or
  same-spelled declarations cannot restore it, and stale or substituted
  semantic bindings reject without relying on accepted-lock replay.

- **COMPONENT-SUBSTRATE.** Implement independently selected component closure
  while keeping deployment/update policy in Cathedral. Componentization must
  bind exact imports, exports, services, mappings, stack demand, leases, and
  installed provider closure. Until that carrier is complete, every
  `Independent` selection fails at one explicit fence.

- **FFIVAL.** After the generic callback/runtime path closes, run the Windows
  `user32` boundary-coherence canary with no raw function pointer or Win32-only
  compiler escape.

- **WIRE-RUNTIME-AND-INSTALLATION.** Complete reusable artifact validation,
  consumed placement authority, W^X/coherence, physical invocation, and
  uninstall/replacement joins. Keep arbitrary runtime bytes-to-code, JIT, and
  raw executable addresses unsupported.

## Platform-gated verification

- Run Linux host/time/filesystem and `IntegerAt` runtime paths on AArch64;
  cross-target compilation is not runtime verification.
- Build and run the Windows GUI callback canary only through the generic ENT4
  path.
- Keep unavailable hosts structurally tested and report the missing runtime leg
  explicitly.
