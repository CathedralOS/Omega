# Tasks

Last pruned: 2026-08-07.

This file is the current execution queue, not a changelog. Commits, canaries,
architecture pages, and design briefs retain completed implementation history.
A task belongs here only when it names:

- the remaining work;
- the owning design and code area;
- a real blocker, if one exists; and
- a concrete acceptance condition.

Before taking a task, fetch `main`, inspect the newest commits in that lane, and
avoid overlapping another active change. Commit and push coherent milestones.
Engineering difficulty is not a design blocker. Unresolved owner decisions live
in `OWNER_QUESTIONS.md`; deliberately deferred research stays at the end of this
file.

## Ownership firewall

Psi operates on Omega files and owns parsing plus all target-neutral semantics
through terminal Psi. Omega consumes terminal Psi and owns provider
installation, optimization, ABI/storage realization, native emission, and
general execution machinery. Target backends own unavoidable ISA, ABI,
object-format, and relocation encoding. Cathedral owns OS data structures,
policies, protocols, and lifecycle.

If Cathedral cannot express a subsystem, identify the missing general Omega
primitive or mark the slice blocked. Do not implement page tables, descriptor
tables, schedulers, process tables, timer queues, or drivers as compiler-owned
Rust models.

Compiler validation and code generation may consume general plans. They must
not acquire customer-shaped semantic types, lifecycle states, writers,
scanners, or receipts.

## Execution order

The numbered groups express dependency order, not an exclusive assignment.
Independent compiler lanes may proceed in parallel when their files and
semantic owners do not overlap.

### P1 — Authority, content, and admitted roots

Owners:

- `wiki/design_briefs/authority_values_and_boundary_evidence.md`
- `wiki/design_briefs/canonical_ir_fuel_and_resource_provisioning.md`
- `wiki/language_guide/chapter_8_domains.md`

Remaining:

- **ENTRY-CONTENT-ROOTS.** Finish target-declared typed slots around the live
  ordinary `builder.roots.bind(target::ProgramEntry, Exact::machine)` binding
  and exact backend entry selection. Hosted free/receiver-bound source-shape
  checks, receiver ZII validation, and fail-closed rejection of bindings owned
  by a non-selected target profile are live. A target profile owns each slot
  identity, schema, direction, lifecycle, cardinality, and exact-requirement,
  complete-conformance, or entry-machine binding shape; `build.omg` names the
  exact entry machine and performs no discovery. Let a target entry schema
  expose only the parameters its program author must handle. A hosted schema
  normally exposes none; a freestanding schema may expose admitted image and
  initial-storage roots.
  Generate the physical bridge from the target's arrival requirement and
  selected calling policy, derive and compose the bridge's complete contract, and call the bound
  entry through its declared source shape. A free entry gets no implicit state.
  An entry with one `&mut self` receiver gets exactly one ZII-valid receiver,
  provisioned beneath an admitted entry storage root and lent only for that
  activation. Record its target-selected image or runtime-storage placement,
  derive image sections as subextents, and allocate later frames/task stacks
  from existing roots. Migrate the corpus and remove the compatibility fallback
  that still recognizes `main`/`Main::run` only when no root binding exists. Do
  not recognize a unique export by convention, and do not introduce ambient
  `static` storage.
- **CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS.** Connect a real
  content-bearing source program to the existing terminal-Psi rows. Add sealed
  content-introduction and custody-exit frontier rows; derive residual geometry
  for partial bodyless boundaries and admit only provider custody acceptance.
  Infer only identity-preserving reshuffles; partition-changing primitives must
  author a theorem and wrappers may compose it.
- **ROOT-INTRODUCTION-AND-BACKING.** Give every content-capable root one internal
  algebra account and classify each fresh establishment occurrence from its
  authority source: compiler-provisioned sealed declared capacity is
  program-local; selected admitted issuance is provider-backed. A checked
  runtime establishment may expose or transform an existing account but never
  originate one. Keep nominal data and algebra denominators free of origin
  policy. Record exact route, capacity, lineage,
  qualification, backing identity, and provenance per root. An operation that
  realizes content against an external substrate must identify an exact
  qualified root and carry correspondence to the same selected provider;
  matching denominator arithmetic alone grants no authority. Report modeled
  identity coverage and reject cross-root recomposition.
- **BOUNDARY-ISSUANCE — depends on the conservation work above.** Derive
  per-invocation geometry from ordinary parameters, entry places, and returned
  values. Retain external ownership, fresh issuance, custody delegation,
  aliasing class, and partitioned succession separately. Provider assertions
  may attest custody; they may not supply computable interval arithmetic.
- Finish routed task-claim establishment, stack-resource authority,
  cancellation conformance, and transactional custody under TR3-TR8. Deferred
  acknowledgements lease the installed interrupt root and controller
  configuration; reconfiguration drains them rather than revoking them.

Acceptance: reconstructed carriers mint no authority; every introduced content
claim traces to compiler-provisioned sealed local capacity or admitted provider
issuance; external effects have an exact root-to-provider backing chain;
partition and residual arithmetic are compiler-derived; overlapping children,
gaps without a custody exit, algebra drift, receipt replay, and cross-root
recomposition reject.

### P2 — Source-visible materialization and placed access

Owners:

- `wiki/design_briefs/programmable_layouts.md`
- `wiki/design_briefs/os_memory_and_hardware_foundation.md`
- `wiki/language_guide/chapter_20_memory_layout_abi.md`

#### L4/L5 — plan-laid views

- Finish source-visible validation/materialization over owned storage.
- Complete non-scalar tiling and mutable-view establishment beyond the live
  record/fixed-array/interior-slice representation checks.
- Keep raw bytes from establishing typed facts without the selected validated
  plan and exact field identities.

#### L6b — `AccessPlan` and `Placed<P, T>`

- Derive borrowed and owned `Placed<P, T>` establishment and retirement from
  `Extent in Granted`; source spelling uses ordinary `&`/`&mut` subrange
  projections, not `ExtentLoan`.
- Implement `Stable` adopt/initialize/validate and `External` adopt. Borrowed
  cleanup ends inside the loan; owned destruction returns
  `Extent in Granted & Vacant` before general allocator/free integration.
- Derive per-field readable, destructive-read, writable, and atomic accessors.
  Preserve logical field extents separately from whole-transfer effect
  footprints. Destructive reads and RMW reserve the complete affected transfer
  unit.
- Enforce per-operation representation and transfer laws: total decode for
  externally readable fields, total/value-proved encode for writes, exact
  provider-supported width/alignment, and operation-specific atomic laws.
  External multi-transfer reads, synthesized RMW, and External initialization
  remain rejected.
- Keep admission polarity (Omega-view alias exclusion) separate from access
  permission. Access-plan rights authorize External reads/writes; `&mut` must
  not falsely claim exclusivity against the device.
- Connect x86-64/AArch64 emission for admitted whole-container External and
  atomic transfers. Publish one sealed core requirement per atomic operation;
  missing conformance means the operation is unavailable.
- Retain schema/device correspondence, optional runtime revision evidence, and
  provider-instance identity separately from storage compatibility.

#### L6c — symbolic materialization

- Carry symbolic sources, placement constraints, immutable post-handoff bytes,
  exact footprint, and invocation plan through final artifacts.
- Connect final placed fragments to source-level provider invocation after
  materialization establishment. Provider preparation must not generate host
  code.
- Bind validation to exact final bytes and placement; fingerprints remain
  report/cache identity, never authority.

Acceptance: UART/MMIO, shared-page IPC, and ordinary RAM use one extent/layout
foundation with different profiles. Misalignment, insufficient rights,
unplanned offsets, narrow External writes, destructive reads through a
repeatable accessor, overlapping transfer footprints, forged profile evidence,
and unsupported atomic operations reject before code generation.

### P3 — Terminal Psi, proof-carrying artifacts, and fuel

Owners:

- `wiki/architecture/pipeline/terminal_psi.md`
- `wiki/design_briefs/canonical_ir_fuel_and_resource_provisioning.md`
- `wiki/architecture/bootstrap_lattice/proof_kernel.md`

Remaining:

- **PSIIR.** Grow terminal production in obligation-complete vertical slices:
  operation semantics, generated obligations, sound proof rules, interpreter
  behavior, Omega lowering, canonical encoding, and fuel identity land
  together. Add general blocks, calls, aggregate values, structural places,
  cleanup/transfer actions, and boundary operations without restoring an
  Omega-to-Psi bridge. Rooted acyclic integer graphs now lower computed
  expressions on unconditional jump bindings and exact-type integer comparison
  guards (including normalized greater forms and recursive integer operands)
  through verification, interpretation, and fuel. Non-crashing shapes also
  reach both native targets. Computed conditional-edge bindings now lower into
  synthesized arm-local blocks, so only the selected expression is evaluated
  and charged, and reach the same native lanes. Boolean-result integer
  comparisons now accept that recursive scalar vocabulary as both operands and
  likewise cross interpretation and native execution. Nested integer-result
  guards now retain recursive short-circuit `&&`/`||` control in reserved
  decision blocks; computed successor bindings remain arm-local, only executed
  tests and edges consume fuel, and non-crashing forms reach both native targets.
  Rooted acyclic Boolean-result graphs now likewise support nested selections,
  convergent tails, recursive non-short-circuit unconditional bindings,
  recursive short-circuit returns, verification, exact fuel, and both
  native targets. Short-circuit Boolean jump bindings now converge through
  value-producing decision blocks, including ordered multi-value tuples whose
  elements evaluate left-to-right in staged blocks while carrying earlier
  results to the authored target; an unconditional multi-value graph no longer
  needs an artificial selector to reach the general lowerer. Computed Boolean
  conditional-edge bindings use the same arm-local construction, including
  short-circuit tuples, so the unselected payload is neither evaluated nor
  charged. Compile-known values now propagate over the lowered acyclic Boolean
  graph, meet conservatively at joins, and reject an unrelated reflexive
  contract without re-reading source expressions. The integer graph uses the
  same lowered-DAG evaluation for recursive arithmetic and integer-comparison
  selectors; any reachable crash exit conservatively suppresses a total result.
  Integer-result graphs now also compute non-short-circuit Boolean
  unconditional bindings (literal, negation, Boolean equality/inequality, and
  exact-type integer comparison), preserve them across terminal block
  parameters, and use the resulting recursive Boolean target expression as
  native control on both architectures. Mixed-scalar short-circuit bindings
  now lower through typed left-to-right tuple stages on unconditional and
  conditional edges; selected-path fuel preserves `&&`/`||` bypass, an
  unselected conditional payload is not executed or charged, and both forms
  reach both native targets. Compile-known propagation now carries typed
  Boolean and integer scalar facts through those bindings, follows the
  resulting selector, meets conservatively at joins, and rejects an unrelated
  closed integer contract. Every scalar-result machine now enters one general
  typed DAG producer, including contract-free all-crash graphs, one-state
  returns, pure unconditional graphs, and three-state conditionals.
  Boolean-result graphs may carry and compute mixed Boolean/integer bindings,
  preserve short-circuit returns, and retain checked crash leaves. The
  duplicate direct-parameter, comparison, Boolean-return, integer-chain,
  three-state conditional, Boolean-chain, Boolean-DAG, and crash-only
  lowerers/builders are retired.
  All-crash graphs intentionally carry no return obligation or proof evidence.
  Checked contract plans now retain the accepted closed Boolean/integer
  requires/ensures equality as a source-handle-free carrier; terminal
  production consumes that carrier and never reopens the typed contract fact.
  Checked proof facts likewise retain the normalized nominal proposition
  declarations and applications consumed by terminal Psi; transparent aliases,
  typed proposition declarations, and proof-fact handles are no longer
  terminal semantic inputs. Checked value facts now also retain every accepted
  executable scalar return, guard, and transition argument as a recursive
  source-handle-free expression keyed by stable state identity and statement
  role. Terminal production consumes those checked expressions and no longer
  reinterprets typed expression nodes. Checked flow facts now retain scalar
  state order, primitive signatures, terminator kind, stable successors, and
  argument arity as a source-handle-free control plan; terminal semantic and
  proof production no longer reads typed statements or transitions. Those
  source tables remain only behind replaceable debug-map construction. Checked
  flow facts also retain stable machine names and the bootstrap signature-
  eligibility decision, so semantic machine selection no longer walks or
  reclassifies typed machine declarations. Finally, an optional checked debug
  plan retains stable subject spans and source-file presentation independently
  from semantic/proof data. The live scalar producer imports no typed-tree
  vocabulary and produces the same semantic module, proof bundle, and debug map
  after the complete typed frontend root is discarded; omitting the debug plan
  yields the same artifact semantics with no debug map. Fixed-width integer
  `~` now follows that same retained checked-expression lane through terminal
  Psi v25, canonical semantic/proof sections, exact verification, fuel,
  artifact-root interpretation, Omega lowering, and x86-64/AArch64 native
  emission. Same-carrier integer arithmetic-policy casts now retain their
  operands source-independently, select enclosing Wrapping/Saturating
  operations, and erase before terminal execution and fuel; direct policy
  erasure remains an ordinary parameter return. Cross-carrier and declared
  semantic-domain casts still fail closed rather than disappearing as
  identities. Source unary integer negation now keeps the parser's settled
  `0 - value` meaning: checked retention contextually lands only that generated
  zero at the validated operand carrier, then the existing Wrapping or
  Saturating subtraction vocabulary crosses artifacts, fuel, interpretation,
  and both native targets. Universally total fixed-width `i*`/`u*` widening
  whose target contains the complete source range now crosses the same
  retained-expression lane as terminal Psi v26:
  canonical semantic/proof sections, exact verification, one-unit operation
  fuel, artifact-root interpretation, Omega lowering, and sign- or
  zero-extending x86-64/AArch64 emission. Same-carrier casts remain static
  retags. A compile-known exact fixed-integer cast whose literal is
  representable in the target now re-lands as an ordinary target-typed terminal
  constant, with no cast operation or extra fuel. Terminal Psi v28 now carries
  proof-gated nonliteral exact fixed-integer casts. The existing validation
  range engine records each accepted occurrence interval in checked facts; the
  terminal operation carries its own obligation identity, and the verifier
  independently reconstructs the stricter target bounds from exact source and
  target carriers. The first source slice derives a true-edge exact-type
  integer comparison, substitutes compile-known constants, rewrites the fact
  through arm-local block parameters, and requires a certificate at the cast
  site. Missing evidence, an unproved path, address involvement, a redundant
  widening, or a same-carrier no-op rejects. Canonical semantic/proof sections,
  one-unit fuel, artifact interpretation, Omega lowering, and x86-64/AArch64
  emission are live. More complex nonliteral range proofs continue to fail
  closed when the independent terminal verifier cannot reconstruct them.
  Terminal Psi v29 carries proof-gated Exact fixed-integer right shift.
  Checked retention preserves a nonliteral `value >> count` only where the
  existing range checker proves `0 <= count < value_width`; the operation owns
  a dedicated obligation, and the verifier reconstructs the necessary lower
  and upper bounds from exact value/count carriers and path-local terminal
  facts. Proof format v20 carries the exact-right-shift term. Canonical artifacts,
  one-unit operation fuel, artifact interpretation, Omega lowering, and
  logical/arithmetic x86-64/AArch64 emission are live. Missing evidence and an
  out-of-range path reject. Terminal Psi v30 now carries proof-gated Exact
  fixed-integer left shift. Source validation independently proves the
  mathematical result interval and rejects a legal-count shift whose value may
  overflow; signed extrema and the `-1 << 63` boundary are covered without
  host-overflow shortcuts. The terminal verifier reconstructs one
  operation-owned conjunction containing count legality plus a distinct
  no-overflow bound. When prior terminal facts determine one exact legal count
  or a finite legal count ceiling, the verifier reconstructs the
  carrier-tight shifted minimum/maximum for that maximum count and retains the
  exact terminal ceiling in the certificate proposition. Otherwise it uses
  carrier-only worst-count bounds (`value <= 1` unsigned, `-1 <= value <= 0`
  signed). Joint value/count relations that cannot be reduced to one rectangular
  ceiling still fail closed. Proof format v21,
  canonical semantic v30 bytes, one-unit fuel, artifact interpretation, Omega
  lowering, and x86-64/AArch64 emission are live.
  Terminal Psi v27 now retains `addr` as a distinct unsigned
  address carrier with its current 64-bit representation rather than collapsing
  it into `u64`; canonical semantic bytes, proof format v18 terms, verification,
  artifact-root interpretation, Omega lowering, and full-width native integer
  comparison preserve that identity. Cross-carrier casts between `addr` and
  fixed integers remain fail-closed. These are implementation frontiers, not
  unresolved language rulings.
- Retire the legacy backend lane as terminal-Psi vocabulary and consumers grow;
  do not restore any `ExpressionHandle` or source-tree dependency in the live
  scalar terminal path.
- **CRASH-CONTRACT.** Source currently parses fingerprinted legacy
  `crashes Cause Scope` buckets, including multiple alternative route facts and
  the unconditional `crashes Cause` shorthand, and preserves explicit
  `crash Cause;` exits through
  checked trees. Source production lowers crash-only machines covered by one
  unconditional same-cause bucket and acyclic scalar control whose crash
  branch has one checker-proved incoming-path bucket, including the live
  structural implication rules; verification and direct
  interpretation retain cause, the legacy scope fields, the canonical
  abandonment-frontier lower bound, and the non-replayable outcome. The scope
  fields do not prove survivor safety. Contextual statement
  `trap` is retired, and the legacy native
  state-graph path rejects explicit crash exits rather than treating them as
  ordinary termination. Route facts are checked as Boolean expressions and do
  not enter requires/ensures proof entailment. Public machine-contract and
  generic-template fingerprints currently canonicalize each legacy
  `(cause, scope)` bucket:
  clause grouping, ordering, and duplicate routes are irrelevant, while an
  unconditional route (including an explicit `true`) subsumes guarded
  alternatives. Checked machine-contract plans now retain that published set
  as source-handle-free crash buckets, fingerprints consume the same carrier,
  contract manifests expose it, and terminal lowering reads it rather than
  reinterpreting typed clauses. The independent checked body layer now retains
  every explicit crash site's state-local location and cause without placing
  that implementation evidence in the public fingerprint; reports expose the
  rows separately and terminal production requires the matching row. Canonical
  published buckets now have dense checked-plan identities, and each site
  records every unconditional same-cause bucket already proven to cover any
  path guard; terminal production consumes that checked join instead of
  searching the published routes. Exact retained incoming path guards and
  their fallthrough negations now join to identical normalized published
  alternatives. The first broader implication slice also derives every
  positive-conjunction conjunct and every negated-disjunction consequence,
  including nested logical negation, comparison operand reversal, and
  equality/inequality negation, while rejecting unsound converses and ordered
  complements without total-order evidence.
  Checked-integer order relations now compose transitively across positive path
  conjunctions: a chain containing at least one strict edge proves a strict
  endpoint bound, while an all-nonstrict chain proves only a nonstrict bound.
  Opposed nonstrict integer paths now apply antisymmetry and prove endpoint
  equality. A nonstrict integer bound plus endpoint disequality now sharpens to
  the corresponding strict bound. One-sided equality claims and unordered
  floats remain opaque.
  The same source-independent closure feeds explicit sites and checked calls;
  unrelated endpoints and unordered floats do not compose. Each site separately
  retains the canonical conjunction of exact incoming predicates so implication
  evidence does not replace its derived path guard; reports expose that carrier.
  Richer guard entailment remains. Terminal Psi v22–v24 retains legacy damage,
  demand, and context fields. Keep their frozen decoders and validators, but do
  not use those fields as survivor-safety evidence. Introduce the next crash
  schema around cause, route guards, and the abandonment-frontier lower bound;
  normalize current source contracts by cause alone and stop producing legacy
  scope/context fields in new modules.
  Checked ownership also records a canonical stable-claim lower bound
  containing every definitely-live,
  unconditional linear obligation at each site. Exhaustive crash paths abandon
  those claims rather than inventing cleanup or consumption, reports expose
  the lower bound,
  and terminal production rejects any checked identity it cannot map to a dense
  terminal claim. Direct positive case-pattern edges now rebind the guarded
  subject through named-state arguments and add exactly the selected
  conditional entry claim. Single-predecessor guard walks compose that argument
  map through intermediate named states using canonical symbol-rooted places;
  non-place/dynamic-index arguments become unknown rather than falling back to
  rendered source labels. Multi-predecessor meets retain the map only when
  every incoming edge carries the same guard polarity and exact composed
  final-parameter map. Nested conditional claims now enter the crash frontier
  only when source-independent membership evidence proves every case segment
  along their canonical claim path. The ownership join also treats
  exhaustive case runs as exhaustive and removes impossible earlier
  alternatives before comparing arm outcomes. Unknown active cases and nested
  cases without proof at every level remain outside the lower bound. Exact-type
  integer comparison guards now reach terminal control, so the
  already-proved ordered-comparison crash coverage is executable rather than
  stopping at checked evidence. Transitive conjunction coverage now crosses
  the same terminal short-circuit slice through verified direct interpretation.
  Native crash lowering remains closed pending
  represented target crash lowering. Direct calls to local machines with
  published crash ceilings now retain source-independent checked invocation
  rows keyed by state/statement/call ordinal and target contract fingerprint.
  The producer substitutes arguments into canonical route predicates, drops
  only routes the existing evaluator proves false, collapses proved-true routes
  to unconditional alternatives, preserves fully disproved calls as empty
  evidence, and records the caller's exact incoming path conjunction plus a
  separate source-independent structural consequence set; semantic reports
  expose both and the surviving buckets. Same-unit private calls now select a
  conservative monotone checked-body summary over the viable invocation graph:
  each
  explicit site contributes an unconditional cause bucket,
  a site-free leaf contributes positive empty evidence, and resolved nested
  summaries retain a temporary source-independent predicate tree, substitute
  positional arguments at every nonrecursive call edge, and collapse to stable
  predicate identities only when checked call rows are emitted. Recursive SCC
  edges widen to unconditional cause buckets so transformed recursive
  arguments cannot generate an infinite predicate family; components still
  reach a finite conservative fixed point.
  unknown dependencies prune their caller closure rather than erasing a nested
  crash. Published callers now check every surviving call route independently
  against a same-cause caller bucket whose guard is unconditional, exactly
  matches the surviving predicate, or is one of the invocation's retained
  path consequences. Positive conjunction, negated disjunction, nested
  negation, Boolean-literal equality/inequality normalization, comparison
  operand reversal, equality/inequality negation normalization, and checked
  integer strict-order/equality consequences feed that same set without
  replacing the exact conjunction. Ordered-comparison
  negation also normalizes when both operands have checked integer types;
  unknown, user-defined, and unordered-float operands remain opaque. Callable trait requirements
  and unresolved compile-time machine parameters now retain source-independent
  crash-contract capsules: each capsule pins the normalized public crash
  buckets to the complete callable-contract fingerprint, and checked calls
  select and substitute those buckets exactly like local published ceilings. Extend
  path-conditioned guard entailment beyond the live structural rules above.
  Explicit crashes now retain the invariant-bearing data identities for every
  open invariant window in the abandonment-frontier lower bound. Keep that row
  audit-only: it is necessary evidence about known abandoned obligations, not a
  complete damage set and never authority for survivor execution. Separately
  compiled
  imported-artifact capsules are design blocked on the semantic import/export
  carrier, symbol identity, and certificate binding requested by
  `wiki/language_guide/appendix_open_questions.md`; diagnostic JSON is not an
  admissible substitute. Remove current-production dependence on v24 context
  maxima while retaining backward validation for archived modules.
  Generalize guarded source production beyond the live structural implication
  rules and current acyclic integer-control shape.
  Terminal Psi already carries the explicit no-successor terminator and its
  canonical machine-local frontier lower bound; native lowering remains closed
  until target `Trap` and `Abort` lowering exists.
- Re-root the reference interpreter and abstract-operation construction fully
  on decoded, verified terminal Psi. The terminal interpreter and
  terminal-Psi-to-abstract-operation builder now have parallel artifact-root
  entries that canonical-decode semantic/proof section bytes and verify them
  under an explicit admission profile before execution or realization
  planning; no producer-owned module or checked tree crosses either entry.
  Continue replacing the legacy checked-tree vocabulary so the shared
  interpreter/native oracle covers the complete language.
- **PCC verifier closure.** The artifact determines its complete obligation
  set; proof bundles only discharge it. Connect `psi-terminal-verifier` to the
  low-rung proof-kernel calculus and choose one auditable closure recorded in
  the architecture: a low reference artifact verifier, a checked
  obligation-reconstruction derivation, or an explicitly trusted Psi verifier.
  A Psi-hosted proof-kernel port is not by itself this closure.
- **IRFUEL.** Extend the live acyclic entry/segment certificates to loops and
  build-time migration. Add attributed response outcomes only after terminal
  Psi has wait/foreign edges from which the verifier can derive them. Migrate
  Cathedral hard roots and later add native metering that preserves accounting
  provenance. Keep target WCET and wall-clock conversion separate.
- **PROOF-RELEVANCE-MIGRATION:** implement binding-level `[erased]` relevance,
  checked noninterference, erased-stripped layout, and obligation preservation.
  Explicit relevance takes precedence over the transitional “recursive means
  proof-only” classifier; non-layoutable `Type` values remain legal only in
  erased positions. Do not infer carrier relation roles from relevance.
- **EFFECTFUL-TYPED-COMPUTATION:** specify the value/computation judgments
  connecting effectful machines to the future typed proof calculus. Treat both
  migrations as staged semantic work, not prerequisites for extending the
  existing terminal vocabulary.

Acceptance: a canonical terminal artifact can be verified after source and
producer state are discarded; the verifier independently reconstructs every
obligation and rejects missing/extra/mismatched evidence; interpretation and
native execution consume that same verified artifact; proof replacement does
not change semantic identity. Crash sites are never represented as ordinary
terminal transitions or absent cleanup, concrete safe invocations can disprove
all crash routes, and installation rejects fault plans that kill either too
little damaged state or too much context-owned state.

### P4 — Calling plans, final footprints, and callbacks

Owners:

- `wiki/design_briefs/calling_plans.md`
- `wiki/design_briefs/os_memory_and_hardware_foundation.md`
- `wiki/language_guide/chapter_23_inline_assembly.md`

#### ENT2c — normalized ABI lowering

- Remove remaining production paths that reconstruct ABI placement from target
  catalogs instead of consuming the selected `CallPlan + StatePlan`.
- Finish retained foreign-storage custody and provider-owned view invalidation.
  Borrow-derived custody cannot survive return; durable retention consumes an
  owned claim and ends through an explicit protocol receipt.
- Add a focused write-only view model rather than disguising write-only foreign
  access as readable memory.
- Keep named no-plan encoders only as differential oracles. Production layout,
  emission, and relocation must require the retained authoritative plan and
  fail closed when it is absent or incompatible.

#### ENT3 — final state-footprint validation

Remaining:

- finish complete entry/body-region enumeration, including format-owned thunks,
  veneers, and generated stubs;
- derive the complete final register/machine-state union and require exact
  equality with earlier `StatePlan` evidence; and
- set certificate completeness only after every executable byte belongs to one
  validated compiler or admitted-format class. Do not add an
  interrupt-specific or second whole-image decoder.

#### ENT4 — registered callbacks

- Let one named static boundary machine satisfy a foreign callback requirement;
  retain its exact calling/state plans and emit the thunk only from selected
  binding lowering.
- Model durable registration as a linear package value with explicit unregister
  and any required code/component lease.
- Implement the narrow Windows `user32` canary (`RegisterClassEx`,
  `CreateWindowEx`/`WM_NCCREATE`, `GetMessage`, `DispatchMessage`,
  `DefWindowProc`, `DestroyWindow`, `UnregisterClass`) without exposing a raw
  code address.
- Derive `Atomic::interruption_fence` same-context evidence from the installed
  external-root route; reject it elsewhere.

Acceptance: changing a normalized plan changes lowering or rejects; forbidden
state introduced anywhere in final executable text rejects; a registered
callback cannot outlive its registration/code lease or smuggle application
state through a raw address.

### P5 — Cathedral bring-up over general Omega primitives

#### BUMP-ALLOCATOR-CANARY — package allocator

- After P1 conservation is source-usable, implement a package-level bump
  allocator over one qualified `Extent`. Two allocations must coexist; release
  cleans and returns the exact subextent without restoring bump-tail capacity;
  reset succeeds only after full recomposition; finish returns the original
  backing.
- Implement owned `Vec<T>` and then `Vec<u8>::Utf8` only after choosing the
  allocator contract needed for cleanup, authority return, and capacity reuse.

#### Address translation

- Build Cathedral's page-table hierarchy, validation states, installation, and
  teardown in Omega source using `source/drivers/facts/x86_page_table_entry.omg`.
- Use pre-reserved storage for the fixed bootstrap table; dynamic hierarchy
  allocation waits for the package allocator. Do not restore a compiler-owned
  page-table model.

#### Exception roots and first timer

- Materialize fatal/diagnostic entries for every architectural exception before
  enabling interrupts.
- Provision dedicated per-CPU double-fault/NMI/machine-check stacks and one
  non-nesting maskable-IRQ stack class; preserve the selected `StatePlan`.
- Bring up PIT+PIC first and LAPIC as the production provider. The hard root only
  acknowledges, records time, publishes a coalesced wake, and returns; fan-out
  runs in an ordinary task.

Acceptance: QEMU installs Cathedral-owned memory/interrupt structures, reports
timer ticks over owned serial output, and halts between ticks. No
customer-shaped compiler concept is introduced.

## Parallel compiler and language lanes

### Frames, reach, and trust

- **R5:** extend exact inferred may-write summaries and relational candidates
  beyond the live acyclic/cycle-safe statement/value-call coverage. Preserve
  facts outside complete frames and treat opaque or unresolved frames as
  conservative fences. Do not restore an authored `stores` clause.
- **STR/EFX:** finish independent normalization/publication of service reach,
  suspension, blocking, termination, mutation, and trust; retire remaining
  legacy umbrella names after consumers migrate.
- **TPR4/TPR6:** connect progress-profile grants and receipts without putting
  private ranking witnesses into public identity.
- **GR6:** finish qualification/trust consumers and their artifact rows.

Acceptance: contract axes normalize independently, wrappers cannot launder
reach or trust, and private proof improvements do not change public identity.

### Multiplicity, tasks, and execution

- **CML4:** construct `EdgeCleanupPlan` after outgoing-value materialization and
  transfer-map commitment. Add reverse-declaration cleanup, contextual cleanup
  contract checking, structural partial-value cleanup, nominal-drop
  partial-move rejection, repeated-cycle resource composition, and
  conservation/backend-ledger reporting.
- **TR3-TR8:** finish whole-call-graph WCSU derivation, bind exact `StackPlan`
  evidence, reserve fixed nonmoving `StackLease`s, validate preservation and
  cancellation conformances, transfer arguments transactionally, lower
  park/resume, and implement the suspension-safe-loan subset.
- **BLOCKEXEC:** implement an ordinary package-level blocking executor with
  bounded queues, moved custody, linear completion claims, suspension, and
  provider selection. A hung in-process worker cannot be killed safely;
  bounded recovery requires process isolation.

Acceptance: linear debt cannot disappear through cleanup or aggregation;
CPU/thread-restricted activations require selected preservation evidence; task
and allocation handles expose no compiler-owned stack/control storage.

### Propositions, quotients, and mathematics

Owner: `wiki/design_briefs/law_bearing_relations_and_quotients.md`.

Remaining N6/N8 work:

- **SELECTED-WITNESS-EVIDENCE:** bind a selected named
  conformance block to one carrierless proof term that introduction and
  elimination can reopen. Consume its complete normalized requirement map;
  do not infer evidence from attached state names. Blocked on owner Q1 for the
  proof-only introduction/elimination surface and retained term identity.
- Replace the inherited-subject conformance header with the settled name-first
  satisfaction declaration and evidence-binder grammar:
  `Name<Telescope>: [Subject] satisfies Trait { ... }` declares one named
  closed implementation, while `Evidence: Subject satisfies Trait` binds one
  explicitly passed implementation. The subject may be omitted for
  carrierless evidence. Every whole-trait implementation is named; no unique-
  visible, priority, or specificity selection is permitted. Retain the name,
  telescope, optional subject, instantiated trait, and normalized rows in
  semantic identity, and migrate existing `Type satisfies Trait as Name`
  source with a targeted diagnostic.
- Add proof-only selected-conformance projection and by-value carrierless `dyn`
  from the complete conformance-block map. The representation can follow the
  settled two-stratum projection, but source selection/opening is blocked on
  owner Q1.
- Add `Respects` over compiler-derived parallel callable argument telescopes.
  Positions are semantic and source names are debug aliases. Derive the
  representative-dependent domain by semantic dependency, the pointwise input
  relation from the selected quotient relations, and the result relation from
  the requested lifted codomain.
- Add proposition-valued heterogeneous constructor lifts selected for exact
  `(quotient relation, container family)` pairs. Transparent dependent records
  lift in dependency order; coarser earlier-field relations generate checked
  proposition-transport obligations owned by the quotient. Do not add global
  carrier roles or an ambient/default relator.
- Extend R6's typed carrier-family binder so reusable relator traits quantify
  over a constructor and expose proposition-valued members. This is the
  higher-kinded/index-telescope prerequisite already owned by the dependent
  ladder, not a quotient-local parallel abstraction.
- Gate runtime decider derivation when a lifted relation depends on erased
  `Type` content: require checked determination by the runtime projection or
  report the undetermined component.
- Migrate `%` from executable-Boolean relations and suffix law discovery to
  proposition evidence plus explicit selected conformances after the work
  above.
- Expand the checked `Nat`, `Int`, `Rat`, sequence/Cauchy, and approximation
  corpus needed for `Real`; keep `Real` proof-only and core-level.

Acceptance: an admitted axiom cannot license quotient formation; selected
Reflexive/Symmetric/Transitive evidence and every `Respects` proof are explicit;
different witnesses establish one stable proposition identity and eliminate
through its declared interface.

### Float providers

Owner: `wiki/design_briefs/float_semantics.md`.

Remaining F7 work:

- provide feature-qualified x86-64 FMA or a checked binary32/binary64 software
  implementation, then select the generic x86 FMA slots;
- retain equally target-specific semantic-edge evidence for every other
  admitted hardware realization; and
- complete the wider proof/`Real` connection under N6/N8.

Checked-result float/integer conversion remains blocked on the separate
checked-result arithmetic decision listed below.

### Lifetimes, dynamic traits, and build-time evaluation

- Finish general outlives constraints, persistent owners, aggregate borrow
  propagation, parameter-backed storage, broader runtime-indexed expressions,
  state-parameter loan-root rebasing, and exact R5 preservation.
- Continue local borrowed `dyn` lowering from the live nominal-conformance
  selection and descriptor representation. Closed carrier conformance blocks
  now retain inline members and explicit existing-machine references, normalize
  one exact trait-qualified row for every inherited requirement, reject ambient
  attached-machine fallback, validate authored row signatures, and carry the
  selected exact row map into checked `dyn` facts. Inline member body and
  contract calls to sibling requirements route through that same map. Trait
  defaults are now instantiated before resolution as exact per-conformance
  realizations, retain default provenance through checked `dyn` facts, and use
  identical sibling routing, including inherited generic defaults. Exact named
  closed-conformance rows now survive state-graph/control-flow lowering and
  drive dynamic-parameter state calls without attached-machine implementation
  lookup;
  local coercion selections also survive with stable owner coordinates and
  retain their original source place. Direct nonescaping local calls now
  devirtualize through the exact retained row with that source place as the
  concrete receiver, including member-place receivers, without reading an
  unmaterialized descriptor. Dynamic calls now retain exact declaring-trait
  requirement symbols, including inherited statement slots; same-spelled
  inherited requirements reject, checked rows carry no compatibility spelling,
  and backend row matching is symbol-only. Closed-conformance synthesis now
  retains every same-named default overload separately; authored members select
  rows by the complete instantiated parameter/result-domain identity, and
  checked `dyn` selections retain every exact overload row. Finish physical
  descriptor/table materialization for pass-through, rebound, and escaping
  values and make every remaining descriptor adapter consume only retained
  rows. Bare dynamic parameters now retain each eligible closed conformance's
  exact row map through state graph and control flow; bodyless static
  conformances and bare exact-requirement satisfiers never license `dyn`, and
  backend candidate discovery no longer searches attached machines by name.
  Bare dynamic call boundaries require one complete conformance per concrete
  carrier; same-carrier ambiguity rejects unless the parameter names the exact
  conformance.
- Complete hermetic semantic evaluation: invocation-specific crash-route
  refinement, target-semantic capsule, separate semantic result and usage
  identities, deterministic progress, and constant/runtime equivalence.
- Add `Hermetic | Receipted | Volatile` observation ceilings and publish realized
  replay/rebuild provenance separately from source semantics.
- Complete the ordinary `Build` API and package executor: bind dependency aliases
  to exact sources, compile each dependency build against package-scoped
  providers, reject ambient/general filesystem escape, and recheck generated
  Omega under the consuming artifact's runtime ceilings.
- Harden the resolver as a separate authority boundary with revision/content
  verification, archive path containment, expansion limits, scoped destination
  writes, and receipts. Generate the unified dependency/build/trust lock,
  fingerprint imported boundary claims as one package claim set, and invalidate
  root acceptance on any member diff. Release-capable standard providers must be
  hermetic or receipted; volatile observations remain explicit development
  policy and fail source-rebuildable release.

### Components and executable trust

- **FFIVAL:** run the narrow Windows `user32` boundary-coherence slice after
  ENT4, using existing activation, custody, registration, stack, and reach
  machinery.
- **TCBMANIFEST:** finish build-profile selection and derive executable TCB
  metadata from selected-provider closure. Keep known entries separate from
  proved completeness; retain provider/executable/plan identity, execution
  scope, origin, implementation evidence, and independent containment axes.
- Extend separate-compilation artifacts with target/runtime stack needs,
  mapping cohorts, two-sided import/export validation, boundary multiplicity,
  custody receipts, and enumerable state roots. Runtime drain/coexistence,
  migration scheduling, and resource provisioning remain consumer/runtime
  work.
- **REPLACE-OPAQUE:** extend replaceable-component tests beyond the live mapping quarantine,
  manifest union, service handover, callback gateway/unregister, and era-ledger
  slice. Proven quiescence is the only route back to reusable mapping capacity.
- Implement serialized capability attenuation/revocation only after the
  component carrier and custody rules are complete.

### Atomic ordering and device protocols

- **ATOMIC-EVENT-MODEL — design blocked.** Define the formal portable event
  model and x86-64/AArch64 refinement before enabling general protocol
  verification or global-order fences. Placed atomic accessors, checked ISA
  barriers, and installed-root same-context evidence do not wait for it.
- Add sealed provider requirements for DMA publication/acquisition, cache
  maintenance, MMIO notification, and posted-write completion. Every emitted
  requirement must be discharged or reject.
- Bind publication evidence to exact range/write state so intersecting writes
  invalidate it. Acquisition consumes request- and instance-bound completion
  evidence. Terminal Psi retains the actual ordering event; erased proof values
  and generic call effects are not lowering barriers.

### Wire runtime and executable installation

- Extend repeated encode/decode to `Vec<T>` after allocator obligations land.
  Packed scalar decode into `&[T]` remains unsupported because variable-width
  encodings cannot form a zero-copy scalar view.
- Connect retained semantic artifacts to loader/provider execution; implement
  trusted/PCC and final-footprint validators; complete target W^X/coherence
  reporting and uninstall/replacement joins.
- Keep arbitrary runtime bytes-to-code, JIT, and raw executable addresses
  unsupported.

Acceptance: only an admitted reusable artifact plus consumed placement authority
can produce installed code; validation binds exact final bytes and placement.

## Blocked index

These entries are pointers, not duplicate specifications.

- **ATOMIC-EVENT-MODEL:** blocked on the portable atomic axioms and target
  refinement choices in `wiki/language_guide/appendix_open_questions.md`.
- **CHECKED-RESULT-ARITHMETIC:** blocked on whether failure-returning checked
  arithmetic earns a distinct public carrier beyond exact-by-default
  obligations and existing policy families.
- **IMPORTED-CRASH-CAPSULES:** blocked on the separately compiled realization
  artifact, import/export identity, and certificate-binding model in
  `wiki/language_guide/appendix_open_questions.md`.

## Platform-gated verification

- Run the Linux host/time/filesystem and `IntegerAt` metadata paths natively on
  AArch64. x86-64 WSL and cross-target structural coverage already exist; do
  not claim runtime verification without the host.
- Build and run the Windows GUI callback canary through ENT4; do not pass a raw
  code address or add a Win32-only escape.
- Keep unavailable hosts structurally tested and report the missing runtime
  leg explicitly.

## Vertical acceptance slices

- **Allocator:** qualified root -> two live subextents -> cleanup/retirement ->
  exact recomposition -> original root returned.
- **PCC:** canonical Psi -> independently reconstructed obligations -> checked
  proof bundle -> interpretation/native agreement after producer state is gone.
- **OS:** UART/MMIO -> Cathedral page tables -> DMA -> hostile/trusted shared
  pages -> exception/timer entry -> SMP AP bring-up, with no customer-shaped
  compiler primitive.
- **Control state:** checked assembly cannot hide stack/control mutation;
  provider exits match their plans; external loans remain inside their extent;
  parked continuations remain non-addressable.

## Deferred until a real customer

- fault-tolerant component restart: define closed-custody component closure,
  explicit owner-death protocols for shared resources, external device reset or
  transaction obligations, and target-supplied isolation evidence together;
  abandonment-frontier reports alone must never license survivors;
- concurrent whole-system composition proofs for deadlock, starvation, memory,
  and response bounds;
- richer measured-recursion guards and multi-subject lexicographic cycles;
- reduced-rational divisibility theory beyond current quotient work;
- asynchronous extent revocation beyond provider quiescence;
- non-blocking executable-visibility tokens;
- runtime-generated host code, JIT, and arbitrary self-modifying code;
- independent final-byte CFI certificates and optional
  CET/PAC/shadow-stack hardening;
- universe levels before a full math-library replay goal;
- reusable fragmented allocation until a growable-container/backend customer
  states its retirement, authority-return, and immediate-reuse demands; and
- an optimizing SSA/register-allocation/SIMD backend beyond current correctness
  requirements.
